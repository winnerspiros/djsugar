// Included at the end of rekordboxfeature.cpp on Android, so the functions
// declared in rekordboxandroid.h can use the ANLZ reader of that file
// without changing it.

#include <QAbstractProxyModel>
#include <QFileInfo>
#include <QSqlError>
#include <QSqlQuery>
#include <QTimer>
#include <algorithm>
#include <cmath>
#include <exception>
#include <vector>

#ifdef __HID__
#include "controllers/android.h"
#endif
#include "library/rekordbox/rekordboxandroid.h"
#include "mixer/basetrackplayer.h"
#include "mixer/playermanager.h"
#include "util/datetime.h"

namespace mixxx {
namespace android {

namespace {

RekordboxPlaylistModel* rekordboxPlaylistModel(QAbstractItemModel* pModel) {
    // The track lists of the QML UI wrap the model of the feature
    while (pModel) {
        if (auto* pRekordboxModel = qobject_cast<RekordboxPlaylistModel*>(pModel)) {
            return pRekordboxModel;
        }
        auto* pProxyModel = qobject_cast<QAbstractProxyModel*>(pModel);
        pModel = pProxyModel ? pProxyModel->sourceModel() : nullptr;
    }
    return nullptr;
}

} // namespace

bool isRekordboxPlaylist(QAbstractItemModel* pModel) {
    return rekordboxPlaylistModel(pModel) != nullptr;
}

std::optional<QList<RekordboxTrack>> rekordboxPlaylistTracks(QAbstractItemModel* pModel) {
    RekordboxPlaylistModel* pRekordboxModel = rekordboxPlaylistModel(pModel);
    if (!pRekordboxModel) {
        return std::nullopt;
    }
    // All tracks, not only those matching the search
    const QString searchText = pRekordboxModel->currentSearch();
    if (!searchText.isEmpty()) {
        pRekordboxModel->search(QString());
    }
    while (pRekordboxModel->canFetchMore(QModelIndex())) {
        pRekordboxModel->fetchMore(QModelIndex());
    }
    QList<std::pair<int, RekordboxTrack>> tracks;
    for (int row = 0; row < pRekordboxModel->rowCount(); ++row) {
        const QModelIndex index = pRekordboxModel->index(row, 0);
        const auto field = [&](ColumnCache::Column column) {
            return pRekordboxModel->getFieldVariant(index, column);
        };
        bool ok = false;
        int position = field(ColumnCache::COLUMN_PLAYLISTTRACKSTABLE_POSITION).toInt(&ok);
        if (!ok) {
            position = row;
        }
        tracks.append({position,
                RekordboxTrack{
                        field(ColumnCache::COLUMN_TRACKLOCATIONSTABLE_LOCATION).toString(),
                        field(ColumnCache::COLUMN_REKORDBOX_ANALYZE_PATH).toString(),
                        field(ColumnCache::COLUMN_LIBRARYTABLE_KEY).toString(),
                        field(ColumnCache::COLUMN_LIBRARYTABLE_COLOR)}});
    }
    if (!searchText.isEmpty()) {
        pRekordboxModel->search(searchText);
    }
    // The order of the playlist, whatever the list is sorted by
    std::stable_sort(tracks.begin(), tracks.end(), [](const auto& a, const auto& b) {
        return a.first < b.first;
    });
    QList<RekordboxTrack> result;
    result.reserve(tracks.size());
    for (auto& track : tracks) {
        result.append(std::move(track.second));
    }
    return result;
}

QDateTime rekordboxDateAdded(const QSqlDatabase& database, const QString& location) {
    QSqlQuery query(database);
    query.prepare(QStringLiteral("SELECT datetime_added FROM ") + kRekordboxLibraryTable +
            QStringLiteral(" WHERE location=:location"));
    query.bindValue(QStringLiteral(":location"), location);
    if (!query.exec() || !query.next() || query.value(0).isNull()) {
        return QDateTime();
    }
    return mixxx::convertVariantToDateTime(query.value(0));
}

TrackId libraryTrackId(const QSqlDatabase& database, const QString& location) {
    QSqlQuery query(database);
    query.prepare(QStringLiteral(
            "SELECT library.id FROM library "
            "INNER JOIN track_locations ON library.location = track_locations.id "
            "WHERE track_locations.location=:location"));
    query.bindValue(QStringLiteral(":location"), location);
    if (!query.exec() || !query.next()) {
        return TrackId();
    }
    return TrackId(query.value(0));
}

bool setTrackDateAdded(
        const QSqlDatabase& database, TrackId trackId, const QDateTime& dateAdded) {
    if (!trackId.isValid() || !dateAdded.isValid()) {
        return false;
    }
    QSqlQuery query(database);
    query.prepare(QStringLiteral("UPDATE library SET datetime_added=:date WHERE id=:id"));
    query.bindValue(QStringLiteral(":date"), dateAdded);
    query.bindValue(QStringLiteral(":id"), trackId.toVariant());
    if (!query.exec()) {
        qWarning() << "Unable to set the date added of track" << trackId << query.lastError();
        return false;
    }
    return true;
}

namespace {

/// Rekordbox stores the beats of a constant tempo in whole milliseconds,
/// which makes a beatgrid of a varying tempo for Mixxx, on which the beat
/// based edits (move by a part of a beat, double or halve) don't work.
/// Replaces such a beatgrid with the constant tempo that fits it, if every
/// beat is within the rounding of Rekordbox.
void straightenRekordboxBeats(const TrackPointer& pTrack) {
    const mixxx::BeatsPointer pBeats = pTrack->getBeats();
    if (!pBeats || pBeats->hasConstantTempo() ||
            pBeats->getSubVersion() != mixxx::rekordboxconstants::beatsSubversion) {
        return;
    }
    const mixxx::audio::SampleRate sampleRate = pBeats->getSampleRate();
    const auto endPosition = mixxx::audio::FramePos(pTrack->getDuration() * sampleRate.value());
    const int count = pBeats->numBeatsInRange(mixxx::audio::kStartFramePos, endPosition);
    if (count < 8) {
        return;
    }
    // Least squares fit of position = first + index * length
    std::vector<double> positions;
    positions.reserve(count);
    auto it = pBeats->iteratorFrom(mixxx::audio::kStartFramePos);
    for (int i = 0; i < count; ++i, ++it) {
        positions.push_back(it->value());
    }
    const double meanIndex = (count - 1) / 2.0;
    double meanPosition = 0;
    for (double position : positions) {
        meanPosition += position;
    }
    meanPosition /= count;
    double covariance = 0;
    double variance = 0;
    for (int i = 0; i < count; ++i) {
        covariance += (i - meanIndex) * (positions[i] - meanPosition);
        variance += (i - meanIndex) * (i - meanIndex);
    }
    const double beatLength = covariance / variance;
    const double firstPosition = meanPosition - meanIndex * beatLength;
    if (beatLength <= 0 || firstPosition < 0) {
        return;
    }
    // Rounded to whole milliseconds, plus a margin
    const double maxDeviation = 1.5 * sampleRate.value() / 1000.0;
    for (int i = 0; i < count; ++i) {
        if (std::abs(positions[i] - (firstPosition + i * beatLength)) > maxDeviation) {
            return;
        }
    }
    const auto bpm = mixxx::Bpm(60.0 * sampleRate.value() / beatLength);
    pTrack->trySetBeats(mixxx::Beats::fromConstTempo(sampleRate,
            mixxx::audio::FramePos(firstPosition),
            bpm,
            mixxx::rekordboxconstants::beatsSubversion));
}

} // namespace

void applyRekordboxData(const TrackPointer& pTrack, const RekordboxTrack& rekordboxTrack) {
    // Same as RekordboxPlaylistModel::getTrack(), for the copy of the file
    const QString location = pTrack->getLocation();
    int timingOffset = 0;
    if (location.endsWith(".mp3", Qt::CaseInsensitive)) {
        const int timingShiftCase = mp3guessenc_timing_shift_case(location.toStdString().c_str());
        switch (timingShiftCase) {
#if defined(__MAD__)
        case EXIT_CODE_CASE_A:
        case EXIT_CODE_CASE_D:
            timingOffset = 26;
            break;
#elif defined(__FFMPEG__)
        case EXIT_CODE_CASE_D:
            timingOffset = 26;
            break;
#endif
        default:
            break;
        }
    }

    const mixxx::audio::SampleRate sampleRate = pTrack->getSampleRate();
    const QString& anlzPath = rekordboxTrack.analyzePath;
    const QString anlzPathExt = anlzPath.left(anlzPath.length() - 3) + "EXT";
    // Tracks that were not analyzed have no ANLZ path, which would be the
    // root of the drive
    const bool hasAnlz = QFileInfo(anlzPath).isFile();
    const bool hasAnlzExt = QFileInfo(anlzPathExt).isFile();
    try {
        if (hasAnlzExt) {
            // Beatgrids appear to be only correct in legacy ANLZ file
            if (hasAnlz) {
                readAnalyze(pTrack, sampleRate, timingOffset, true, anlzPath);
            }
            readAnalyze(pTrack, sampleRate, timingOffset, false, anlzPathExt);
        } else if (hasAnlz) {
            // Without an .EXT, the .DAT has the beatgrid and the cues, which
            // readAnalyze() reads in two passes
            readAnalyze(pTrack, sampleRate, timingOffset, true, anlzPath);
            readAnalyze(pTrack, sampleRate, timingOffset, false, anlzPath);
        }
    } catch (const std::exception& e) {
        // A damaged file shouldn't stop copying the playlist
        qWarning() << "Unable to read the Rekordbox analysis" << anlzPath << e.what();
    }
    straightenRekordboxBeats(pTrack);

    // Unlike for a track on the drive, a missing key or color doesn't
    // replace what the file already has
    if (!rekordboxTrack.key.isEmpty()) {
        pTrack->setKeys(KeyFactory::makeBasicKeysNormalized(
                rekordboxTrack.key, mixxx::track::io::key::USER));
    }
    if (const auto color = mixxx::RgbColor::fromQVariant(rekordboxTrack.color);
            color && *color != kColorForIDNoColor) {
        pTrack->setColor(color);
    }
}

namespace {

/// The Rekordbox data of the track with the file `location`, if it is a
/// track of a Rekordbox drive or backup that was opened
std::optional<RekordboxTrack> rekordboxTrackAt(
        const QSqlDatabase& database, const QString& location) {
    QSqlQuery query(database);
    query.prepare(QStringLiteral("SELECT analyze_path, key, color FROM ") +
            kRekordboxLibraryTable + QStringLiteral(" WHERE location=:location"));
    query.bindValue(QStringLiteral(":location"), location);
    if (!query.exec() || !query.next()) {
        return std::nullopt;
    }
    return RekordboxTrack{location,
            query.value(0).toString(),
            query.value(1).toString(),
            query.value(2)};
}

void applyRekordboxDataToTrack(const TrackPointer& pTrack, const QSqlDatabase& database) {
    // The positions of Rekordbox are converted with the sample rate
    if (!pTrack || pTrack->isBpmLocked() || !pTrack->getSampleRate().isValid()) {
        return;
    }
    // Already applied, maybe changed in Mixxx since
    const mixxx::BeatsPointer pBeats = pTrack->getBeats();
    if (pBeats && pBeats->getSubVersion() == mixxx::rekordboxconstants::beatsSubversion) {
        return;
    }
    const std::optional<RekordboxTrack> rekordboxTrack =
            rekordboxTrackAt(database, pTrack->getLocation());
    if (!rekordboxTrack) {
        return;
    }
    qDebug() << "Applying the Rekordbox analysis to" << pTrack->getLocation();
    applyRekordboxData(pTrack, *rekordboxTrack);
}

void connectDeckForRekordbox(
        PlayerManager* pPlayerManager, BaseTrackPlayer* pDeck, const QSqlDatabase& database) {
    // The deck is only connected once
    if (!pDeck || pDeck->property("androidRekordboxData").toBool()) {
        return;
    }
    pDeck->setProperty("androidRekordboxData", true);
    // loadingTrack is emitted before the engine loads the track and before
    // the analysis of the loaded track starts
    QObject::connect(pDeck,
            &BaseTrackPlayer::loadingTrack,
            pPlayerManager,
            [database](TrackPointer pNewTrack, TrackPointer) {
                applyRekordboxDataToTrack(pNewTrack, database);
            });
}

void connectDecksForRekordbox(PlayerManager* pPlayerManager, const QSqlDatabase& database) {
    for (int i = 0; i < pPlayerManager->numberOfDecks(); ++i) {
        connectDeckForRekordbox(pPlayerManager, pPlayerManager->getDeckBase(i), database);
    }
}

} // namespace

void applyRekordboxDataOnLoad(PlayerManager* pPlayerManager, const QSqlDatabase& database) {
    connectDecksForRekordbox(pPlayerManager, database);
    QObject::connect(pPlayerManager,
            &PlayerManager::numberOfDecksChanged,
            pPlayerManager,
            [pPlayerManager, database](int) {
                connectDecksForRekordbox(pPlayerManager, database);
            });
}

#ifdef __HID__
namespace {

/// Android mounts a drive a few seconds after it was plugged in
constexpr int kUsbChangeDelayMs = 4000;

/// Listens to USB changes as long as its feature exists
class UsbDeviceWatcher : public QObject {
  public:
    explicit UsbDeviceWatcher(RekordboxFeature* pFeature)
            : QObject(pFeature) {
        m_timer.setSingleShot(true);
        m_timer.setInterval(kUsbChangeDelayMs);
        QObject::connect(&m_timer, &QTimer::timeout, pFeature, &RekordboxFeature::refreshDevices);
        // Called on a Java thread
        m_listenerId = mixxx::android::addDevicesChangedListener([this]() {
            QMetaObject::invokeMethod(
                    this, [this]() { m_timer.start(); }, Qt::QueuedConnection);
        });
    }
    ~UsbDeviceWatcher() override {
        // Waits for a callback that is running
        mixxx::android::removeDevicesChangedListener(m_listenerId);
    }

  private:
    QTimer m_timer;
    int m_listenerId;
};

} // namespace

void watchUsbDevices(RekordboxFeature* pFeature) {
    new UsbDeviceWatcher(pFeature);
}
#else
// The USB notifications come with the controller support
void watchUsbDevices(RekordboxFeature*) {
}
#endif

} // namespace android
} // namespace mixxx

void RekordboxFeature::refreshDevices() {
    if (m_devicesFuture.isRunning()) {
        // Looks again once the current search is done
        QTimer::singleShot(1000, this, &RekordboxFeature::refreshDevices);
        return;
    }
    qInfo() << "USB devices changed, looking for Rekordbox devices again";
    m_devicesFuture = QtConcurrent::run(findRekordboxDevices);
    m_devicesFutureWatcher.setFuture(m_devicesFuture);
}
