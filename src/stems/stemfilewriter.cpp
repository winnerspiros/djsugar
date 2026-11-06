#include "stems/stemfilewriter.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QtEndian>
#include <algorithm>
#include <thread>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
}

namespace mixxx {
namespace stems {

namespace {

struct StemDescription {
    const char* name;
    const char* color;
};

// In the order of the separators, with the default colors of Mixxx
constexpr StemDescription kStems[kStemCount] = {
        {"Drums", "#009E73"},
        {"Bass", "#D55E00"},
        {"Other", "#CC79A7"},
        {"Vocals", "#56B4E9"},
};

QByteArray stemManifest() {
    QJsonArray stems;
    for (const auto& stem : kStems) {
        stems.append(QJsonObject{
                {QStringLiteral("name"), QString::fromLatin1(stem.name)},
                {QStringLiteral("color"), QString::fromLatin1(stem.color)},
        });
    }
    const QJsonObject disabled{{QStringLiteral("enabled"), false}};
    const QJsonObject manifest{
            {QStringLiteral("version"), 1},
            {QStringLiteral("mastering_dsp"),
                    QJsonObject{
                            {QStringLiteral("compressor"), disabled},
                            {QStringLiteral("limiter"), disabled},
                    }},
            {QStringLiteral("stems"), stems},
    };
    return QJsonDocument(manifest).toJson(QJsonDocument::Compact);
}

QByteArray box(const char* type, const QByteArray& payload) {
    QByteArray result(8, '\0');
    qToBigEndian<quint32>(static_cast<quint32>(8 + payload.size()), result.data());
    std::copy(type, type + 4, result.data() + 4);
    return result + payload;
}

QString ffmpegErrorString(int error) {
    char buffer[AV_ERROR_MAX_STRING_SIZE] = {};
    av_strerror(error, buffer, sizeof(buffer));
    return QString::fromUtf8(buffer);
}

} // namespace

StemFileWriter::StemFileWriter()
        : m_pFormatContext(nullptr) {
    for (auto& stream : m_streams) {
        stream.pFrame = av_frame_alloc();
    }
}

StemFileWriter::~StemFileWriter() {
    close();
    for (auto& stream : m_streams) {
        av_frame_free(&stream.pFrame);
    }
}

bool StemFileWriter::fail(const QString& message, int ffmpegError) {
    m_error = ffmpegError ? message + QStringLiteral(": ") + ffmpegErrorString(ffmpegError)
                          : message;
    return false;
}

bool StemFileWriter::streamFail(int streamIndex, const QString& message, int ffmpegError) {
    m_streams[streamIndex].error = ffmpegError
            ? message + QStringLiteral(": ") + ffmpegErrorString(ffmpegError)
            : message;
    return false;
}

bool StemFileWriter::open(const QString& path,
        int sampleRate,
        int bitrateKbps,
        const QMap<QString, QString>& tags) {
    m_path = path;
    const QByteArray pathUtf8 = path.toUtf8();
    int result = avformat_alloc_output_context2(
            &m_pFormatContext, nullptr, "mp4", pathUtf8.constData());
    if (result < 0 || !m_pFormatContext) {
        return fail(QStringLiteral("Unable to create the MP4 file"), result);
    }
    // The Fraunhofer encoder sounds better than the native one
    const AVCodec* pCodec = avcodec_find_encoder_by_name("libfdk_aac");
    if (!pCodec) {
        pCodec = avcodec_find_encoder(AV_CODEC_ID_AAC);
    }
    if (!pCodec) {
        return fail(QStringLiteral("No AAC encoder"));
    }
    const AVSampleFormat* pSampleFormats = nullptr;
    int sampleFormatCount = 0;
    avcodec_get_supported_config(nullptr,
            pCodec,
            AV_CODEC_CONFIG_SAMPLE_FORMAT,
            0,
            reinterpret_cast<const void**>(&pSampleFormats),
            &sampleFormatCount);
    AVSampleFormat sampleFormat = pSampleFormats && sampleFormatCount > 0
            ? pSampleFormats[0]
            : AV_SAMPLE_FMT_FLTP;
    for (int i = 0; i < sampleFormatCount; ++i) {
        if (pSampleFormats[i] == AV_SAMPLE_FMT_FLTP || pSampleFormats[i] == AV_SAMPLE_FMT_FLT) {
            sampleFormat = pSampleFormats[i];
        }
    }

    for (int index = 0; index < kStreamCount; ++index) {
        AVStream* pStream = avformat_new_stream(m_pFormatContext, nullptr);
        AVCodecContext* pContext = avcodec_alloc_context3(pCodec);
        if (!pStream || !pContext) {
            avcodec_free_context(&pContext);
            return fail(QStringLiteral("Out of memory"));
        }
        m_streams[index].pCodecContext = pContext;
        pContext->sample_fmt = sampleFormat;
        pContext->sample_rate = sampleRate;
        pContext->bit_rate = static_cast<int64_t>(bitrateKbps) * 1000;
        av_channel_layout_default(&pContext->ch_layout, 2);
        pContext->time_base = AVRational{1, sampleRate};
        if (m_pFormatContext->oformat->flags & AVFMT_GLOBALHEADER) {
            pContext->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
        }
        result = avcodec_open2(pContext, pCodec, nullptr);
        if (result < 0) {
            return fail(QStringLiteral("Unable to open the AAC encoder"), result);
        }
        result = avcodec_parameters_from_context(pStream->codecpar, pContext);
        if (result < 0) {
            return fail(QStringLiteral("Unable to set up the stream"), result);
        }
        pStream->time_base = pContext->time_base;
        // Only the mix plays in other players
        pStream->disposition = index == 0 ? AV_DISPOSITION_DEFAULT : 0;
    }

    for (auto it = tags.constBegin(); it != tags.constEnd(); ++it) {
        if (!it.value().isEmpty()) {
            av_dict_set(&m_pFormatContext->metadata,
                    it.key().toUtf8().constData(),
                    it.value().toUtf8().constData(),
                    0);
        }
    }

    result = avio_open(&m_pFormatContext->pb, pathUtf8.constData(), AVIO_FLAG_WRITE);
    if (result < 0) {
        return fail(QStringLiteral("Unable to write %1").arg(path), result);
    }
    result = avformat_write_header(m_pFormatContext, nullptr);
    if (result < 0) {
        return fail(QStringLiteral("Unable to write the MP4 header"), result);
    }
    return true;
}

bool StemFileWriter::write(const float* pMix,
        const std::array<const float*, kStemCount>& stems,
        SINT frameCount) {
    return encodeAll({pMix, stems[0], stems[1], stems[2], stems[3]}, frameCount) &&
            writePackets();
}

bool StemFileWriter::encodeAll(
        const std::array<const float*, kStreamCount>& input, SINT frameCount) {
    std::array<bool, kStreamCount> encoded;
    std::array<std::thread, kStreamCount> threads;
    for (int index = 0; index < kStreamCount; ++index) {
        threads[index] = std::thread([this, &encoded, &input, index, frameCount]() {
            // The models keep the faster cores
            moveToEfficiencyCores();
            encoded[index] = encode(index, input[index], static_cast<int>(frameCount));
        });
    }
    for (auto& thread : threads) {
        thread.join();
    }
    for (int index = 0; index < kStreamCount; ++index) {
        if (!encoded[index]) {
            return fail(m_streams[index].error);
        }
    }
    return true;
}

bool StemFileWriter::writePackets() {
    for (int index = 0; index < kStreamCount; ++index) {
        Stream& stream = m_streams[index];
        AVStream* pStream = m_pFormatContext->streams[index];
        for (AVPacket*& pPacket : stream.packets) {
            av_packet_rescale_ts(pPacket, stream.pCodecContext->time_base, pStream->time_base);
            pPacket->stream_index = index;
            const int result = av_interleaved_write_frame(m_pFormatContext, pPacket);
            av_packet_free(&pPacket);
            if (result < 0) {
                for (auto& pending : stream.packets) {
                    av_packet_free(&pending);
                }
                stream.packets.clear();
                return fail(QStringLiteral("Unable to write the MP4 file"), result);
            }
        }
        stream.packets.clear();
    }
    return true;
}

bool StemFileWriter::encode(int streamIndex, const float* pStereo, int frameCount) {
    Stream& stream = m_streams[streamIndex];
    AVCodecContext* pContext = stream.pCodecContext;
    if (pStereo) {
        stream.pending.insert(stream.pending.end(), pStereo, pStereo + 2 * frameCount);
    }
    const int frameSize = pContext->frame_size > 0 ? pContext->frame_size : 1024;
    size_t consumed = 0;
    // Without input, the last partial frame is flushed
    while (stream.pending.size() - consumed >= static_cast<size_t>(2 * frameSize) ||
            (!pStereo && stream.pending.size() > consumed)) {
        const int samples = static_cast<int>(std::min<size_t>(
                frameSize, (stream.pending.size() - consumed) / 2));
        av_frame_unref(stream.pFrame);
        stream.pFrame->nb_samples = samples;
        stream.pFrame->format = pContext->sample_fmt;
        stream.pFrame->sample_rate = pContext->sample_rate;
        av_channel_layout_copy(&stream.pFrame->ch_layout, &pContext->ch_layout);
        int result = av_frame_get_buffer(stream.pFrame, 0);
        if (result < 0) {
            return streamFail(streamIndex, QStringLiteral("Out of memory"), result);
        }
        const float* pInput = stream.pending.data() + consumed;
        switch (pContext->sample_fmt) {
        case AV_SAMPLE_FMT_FLTP: {
            auto* pLeft = reinterpret_cast<float*>(stream.pFrame->data[0]);
            auto* pRight = reinterpret_cast<float*>(stream.pFrame->data[1]);
            for (int i = 0; i < samples; ++i) {
                pLeft[i] = pInput[2 * i];
                pRight[i] = pInput[2 * i + 1];
            }
            break;
        }
        case AV_SAMPLE_FMT_FLT:
            std::copy(pInput, pInput + 2 * samples, reinterpret_cast<float*>(stream.pFrame->data[0]));
            break;
        case AV_SAMPLE_FMT_S16: {
            auto* pOutput = reinterpret_cast<int16_t*>(stream.pFrame->data[0]);
            for (int i = 0; i < 2 * samples; ++i) {
                pOutput[i] = static_cast<int16_t>(
                        std::clamp(pInput[i] * 32767.0f, -32768.0f, 32767.0f));
            }
            break;
        }
        default:
            return streamFail(streamIndex, QStringLiteral("Unsupported AAC sample format"));
        }
        consumed += static_cast<size_t>(2 * samples);
        stream.pFrame->pts = stream.pts;
        stream.pts += samples;
        result = avcodec_send_frame(pContext, stream.pFrame);
        if (result < 0) {
            return streamFail(streamIndex, QStringLiteral("Unable to encode"), result);
        }
        if (!drain(streamIndex)) {
            return false;
        }
    }
    stream.pending.erase(stream.pending.begin(), stream.pending.begin() + consumed);
    return true;
}

bool StemFileWriter::drain(int streamIndex) {
    Stream& stream = m_streams[streamIndex];
    while (true) {
        AVPacket* pPacket = av_packet_alloc();
        if (!pPacket) {
            return streamFail(streamIndex, QStringLiteral("Out of memory"));
        }
        const int result = avcodec_receive_packet(stream.pCodecContext, pPacket);
        if (result < 0) {
            av_packet_free(&pPacket);
            if (result == AVERROR(EAGAIN) || result == AVERROR_EOF) {
                return true;
            }
            return streamFail(streamIndex, QStringLiteral("Unable to encode"), result);
        }
        stream.packets.push_back(pPacket);
    }
}

bool StemFileWriter::finish() {
    for (int index = 0; index < kStreamCount; ++index) {
        if (!encode(index, nullptr, 0)) {
            return fail(m_streams[index].error);
        }
        const int result = avcodec_send_frame(m_streams[index].pCodecContext, nullptr);
        if (result < 0) {
            return fail(QStringLiteral("Unable to encode"), result);
        }
        if (!drain(index)) {
            return fail(m_streams[index].error);
        }
    }
    if (!writePackets()) {
        return false;
    }
    const int result = av_write_trailer(m_pFormatContext);
    if (result < 0) {
        return fail(QStringLiteral("Unable to finish the MP4 file"), result);
    }
    close();
    return addStemAtom();
}

void StemFileWriter::abort() {
    close();
    if (!m_path.isEmpty()) {
        QFile::remove(m_path);
    }
}

void StemFileWriter::close() {
    for (auto& stream : m_streams) {
        avcodec_free_context(&stream.pCodecContext);
        stream.pending.clear();
        stream.pts = 0;
        for (auto& pPacket : stream.packets) {
            av_packet_free(&pPacket);
        }
        stream.packets.clear();
        stream.error.clear();
    }
    if (m_pFormatContext) {
        if (m_pFormatContext->pb) {
            avio_closep(&m_pFormatContext->pb);
        }
        avformat_free_context(m_pFormatContext);
        m_pFormatContext = nullptr;
    }
}

bool StemFileWriter::addStemAtom() {
    // FFmpeg writes the movie box (moov) last: rewrite it with the stem
    // manifest in its user data box (moov/udta/stem)
    QFile file(m_path);
    if (!file.open(QIODevice::ReadWrite)) {
        return fail(QStringLiteral("Unable to open %1").arg(m_path));
    }
    const qint64 fileSize = file.size();
    qint64 moovOffset = -1;
    qint64 offset = 0;
    while (offset + 8 <= fileSize) {
        file.seek(offset);
        const QByteArray header = file.read(16);
        if (header.size() < 8) {
            break;
        }
        qint64 size = qFromBigEndian<quint32>(header.constData());
        if (size == 1 && header.size() == 16) {
            size = static_cast<qint64>(qFromBigEndian<quint64>(header.constData() + 8));
        } else if (size == 0) {
            size = fileSize - offset;
        }
        if (size < 8) {
            break;
        }
        if (header.mid(4, 4) == "moov") {
            moovOffset = offset;
        }
        offset += size;
    }
    if (moovOffset < 0 || offset != fileSize) {
        return fail(QStringLiteral("Unexpected MP4 layout"));
    }
    file.seek(moovOffset);
    const QByteArray moov = file.read(fileSize - moovOffset);
    if (moov.size() != fileSize - moovOffset) {
        return fail(QStringLiteral("Unable to read the MP4 file"));
    }
    const QByteArray stemBox = box("stem", stemManifest());

    // Children of the movie box, with the stem box added to the user data
    QByteArray children;
    bool hasUserData = false;
    int at = 8;
    while (at + 8 <= moov.size()) {
        const int size = static_cast<int>(qFromBigEndian<quint32>(moov.constData() + at));
        if (size < 8 || at + size > moov.size()) {
            return fail(QStringLiteral("Unexpected MP4 layout"));
        }
        const QByteArray child = moov.mid(at, size);
        if (child.mid(4, 4) == "udta") {
            children += box("udta", child.mid(8) + stemBox);
            hasUserData = true;
        } else {
            children += child;
        }
        at += size;
    }
    if (!hasUserData) {
        children += box("udta", stemBox);
    }
    const QByteArray newMoov = box("moov", children);
    if (!file.resize(moovOffset) || !file.seek(moovOffset) ||
            file.write(newMoov) != newMoov.size()) {
        return fail(QStringLiteral("Unable to write %1").arg(m_path));
    }
    return true;
}

} // namespace stems
} // namespace mixxx
