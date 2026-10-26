#pragma once

#include <memory>

#include "library/browse/browsetablemodel.h"
#include "library/libraryfeature.h"
#include "library/proxytrackmodel.h"
#include "preferences/usersettings.h"

class RecordingManager;
class FolderTreeModel;

class RecordingFeature final : public LibraryFeature {
    Q_OBJECT
  public:
    RecordingFeature(Library* parent,
                     UserSettingsPointer pConfig,
                     RecordingManager* pRecordingManager);
    ~RecordingFeature() override = default;

    QVariant title() override;

    void bindLibraryWidget(WLibrary* libraryWidget,
                    KeyboardEventFilter* keyboard) override;

    TreeItemModel* sidebarModel() const override;

  public slots:
    void activate() override;

  signals:
    void setRootIndex(const QModelIndex&);
    void requestRestoreSearch();
    void refreshBrowseModel();

  private:
    RecordingManager* const m_pRecordingManager;

    FolderTreeModel* m_pSidebarModel;

    // The QML UI has no library widget, so no DlgRecording: there the
    // recordings are shown in its track list with these models (see
    // activate()). The proxy is declared last, to be destroyed first.
    bool m_libraryWidgetBound = false;
    std::unique_ptr<BrowseTableModel> m_pBrowseModel;
    std::unique_ptr<ProxyTrackModel> m_pProxyModel;
};
