#include "library/recording/recordingfeature.h"

#include "controllers/keyboard/keyboardeventfilter.h"
#include "library/browse/foldertreemodel.h"
#include "library/library.h"
#include "library/recording/dlgrecording.h"
#include "moc_recordingfeature.cpp"
#include "recording/recordingmanager.h"
#include "widget/wlibrary.h"

namespace {

const QString kViewName = QStringLiteral("Recording");

} // anonymous namespace

RecordingFeature::RecordingFeature(Library* pLibrary,
        UserSettingsPointer pConfig,
        RecordingManager* pRecordingManager)
        : LibraryFeature(pLibrary, pConfig, QStringLiteral("recordings")),
          m_pRecordingManager(pRecordingManager),
          m_pSidebarModel(new FolderTreeModel(this)) {
}

QVariant RecordingFeature::title() {
    return QVariant(tr("Recordings"));
}

TreeItemModel* RecordingFeature::sidebarModel() const {
    return m_pSidebarModel;
}

void RecordingFeature::bindLibraryWidget(WLibrary* pLibraryWidget,
                                  KeyboardEventFilter *keyboard) {
    m_libraryWidgetBound = true;
    //The view will be deleted by LibraryWidget
    DlgRecording* pRecordingView = new DlgRecording(pLibraryWidget,
                                                    m_pConfig,
                                                    m_pLibrary,
                                                    m_pRecordingManager,
                                                    keyboard);

    pRecordingView->installEventFilter(keyboard);
    pLibraryWidget->registerView(kViewName, pRecordingView);
    connect(pRecordingView,
            &DlgRecording::loadTrack,
            this,
            &RecordingFeature::loadTrack);
    connect(pRecordingView,
            &DlgRecording::loadTrackToPlayer,
            this,
            &RecordingFeature::loadTrackToPlayer);
    connect(this,
            &RecordingFeature::refreshBrowseModel,
            pRecordingView,
            &DlgRecording::refreshBrowseModel);
    connect(this,
            &RecordingFeature::requestRestoreSearch,
            pRecordingView,
            &DlgRecording::slotRestoreSearch);
    connect(pRecordingView,
            &DlgRecording::restoreSearch,
            this,
            &RecordingFeature::restoreSearch);
    connect(pRecordingView,
            &DlgRecording::restoreModelState,
            this,
            &RecordingFeature::restoreModelState);
}

void RecordingFeature::activate() {
    if (!m_libraryWidgetBound) {
        // QML UI: the recordings in its track list, read again each time
        if (!m_pBrowseModel) {
            m_pBrowseModel = std::make_unique<BrowseTableModel>(this,
                    m_pLibrary->trackCollectionManager(),
                    m_pRecordingManager,
                    "mixxx.db.model.recording");
            m_pProxyModel = std::make_unique<ProxyTrackModel>(m_pBrowseModel.get(), true);
            m_pProxyModel->setFilterCaseSensitivity(Qt::CaseInsensitive);
            m_pProxyModel->setSortCaseSensitivity(Qt::CaseInsensitive);
        }
        m_pBrowseModel->setPath(mixxx::FileAccess(
                mixxx::FileInfo(m_pRecordingManager->getRecordingDir())));
        emit showTrackModel(m_pProxyModel.get());
        emit enableCoverArtDisplay(false);
        return;
    }
    emit refreshBrowseModel();
    emit switchToView(kViewName);
    emit requestRestoreSearch();
    emit enableCoverArtDisplay(false);
}
