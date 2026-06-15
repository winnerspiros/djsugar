#pragma once

#include <memory>

#include "preferences/dialog/dlgpreferencepage.h"
#include "preferences/dialog/ui_dlgprefrenderdlg.h"
#include "preferences/usersettings.h"

class DlgPrefRenderer : public DlgPreferencePage, public Ui::DlgPrefRenderDlg {
    Q_OBJECT
  public:
    DlgPrefRenderer(QWidget* pParent, UserSettingsPointer pConfig);
    virtual ~DlgPrefRenderer();

  public slots:
    void slotUpdate() override;
    void slotApply() override;
    void slotResetToDefaults() override;

  private slots:
    void slotSetRendererBackend(int index);

  private:
    void loadBackendList();

    UserSettingsPointer m_pConfig;
};
