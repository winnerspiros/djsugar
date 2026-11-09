#include "sources/youtube/soundsourceyoutube.h"

#include "util/logger.h"

namespace mixxx {

namespace {
const Logger kLogger("SoundSourceYouTube");
} // namespace

const QString SoundSourceProviderYouTube::kDisplayName = "YouTube";

SoundSourceYouTube::SoundSourceYouTube(const QUrl& url)
        : SoundSource(url) {
}

SoundSourceYouTube::~SoundSourceYouTube() {
    close();
}

void SoundSourceYouTube::close() {
}

ReadableSampleFrames SoundSourceYouTube::readSampleFramesClamped(
        const WritableSampleFrames& sampleFrames) {
    return ReadableSampleFrames(sampleFrames.frameIndexRange());
}

SoundSource::OpenResult SoundSourceYouTube::tryOpen(
        OpenMode mode,
        const OpenParams& params) {
    Q_UNUSED(mode);
    Q_UNUSED(params);
    kLogger.info() << "Opening YouTube stream:" << getUrl().toString();

    // SponsorBlock segments are handled per deck by SponsorBlockController
    return OpenResult::Succeeded;
}

} // namespace mixxx
