#pragma once

#include <QMap>
#include <QString>
#include <array>
#include <vector>

#include "stems/stemseparator.h"

struct AVCodecContext;
struct AVFormatContext;
struct AVFrame;
struct AVPacket;

namespace mixxx {
namespace stems {

/// Writes a stem file (.stem.mp4, as made by Native Instruments): an MP4 with
/// an AAC stream of the mix followed by one per stem, and a "stem" atom with
/// the names and colors of the stems
class StemFileWriter {
  public:
    StemFileWriter();
    ~StemFileWriter();

    /// Creates the file. bitrate is the AAC bitrate of each stream in kbps.
    bool open(const QString& path,
            int sampleRate,
            int bitrateKbps,
            const QMap<QString, QString>& tags);

    /// Writes interleaved stereo frames of the mix and of the stems
    bool write(const float* pMix,
            const std::array<const float*, kStemCount>& stems,
            SINT frameCount);

    /// Flushes the encoders and adds the stem atom. The file is complete
    /// when it returns true.
    bool finish();

    /// Closes and deletes an unfinished file
    void abort();

    const QString& error() const {
        return m_error;
    }

  private:
    static constexpr int kStreamCount = kStemCount + 1;

    /// The streams are encoded at the same time, each by its own thread;
    /// their packets are written to the file afterwards, in order
    struct Stream {
        AVCodecContext* pCodecContext = nullptr;
        AVFrame* pFrame = nullptr;
        /// Interleaved stereo samples waiting for a full encoder frame
        std::vector<float> pending;
        int64_t pts = 0;
        /// Encoded, not yet written
        std::vector<AVPacket*> packets;
        QString error;
    };

    /// Encodes into the packets of the stream. Without input, flushes the
    /// last partial frame.
    bool encode(int streamIndex, const float* pStereo, int frameCount);
    /// Takes the packets the encoder has ready
    bool drain(int streamIndex);
    /// Encodes all streams at the same time
    bool encodeAll(const std::array<const float*, kStreamCount>& input, SINT frameCount);
    /// Writes the encoded packets to the file
    bool writePackets();
    bool streamFail(int streamIndex, const QString& message, int ffmpegError = 0);
    bool addStemAtom();
    void close();
    bool fail(const QString& message, int ffmpegError = 0);

    QString m_path;
    AVFormatContext* m_pFormatContext;
    std::array<Stream, kStreamCount> m_streams;
    QString m_error;
};

} // namespace stems
} // namespace mixxx
