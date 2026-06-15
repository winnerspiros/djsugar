#include "dlgprefrenderer.h"

#include <QComboBox>

#include "preferences/rendererbackend.h"
#include "moc_dlgprefrenderer.cpp"

namespace {

const QString kGraphicsGroup = QStringLiteral("[Graphics]");
const ConfigKey kRendererBackendKey(kGraphicsGroup, QStringLiteral("rendererBackend"));

} // namespace

using namespace mixxx;

DlgPrefRenderer::DlgPrefRenderer(QWidget* pParent, UserSettingsPointer pConfig)
        : DlgPreferencePage(pParent),
          m_pConfig(pConfig) {
    setupUi(this);

    loadBackendList();

    connect(comboBackend,
            QOverload<int>::of(&QComboBox::currentIndexChanged),
            this,
            &DlgPrefRenderer::slotSetRendererBackend);
}

DlgPrefRenderer::~DlgPrefRenderer() = default;

void DlgPrefRenderer::loadBackendList() {
    comboBackend->clear();
    QList<RendererBackend> backends = availableRenderers();
    for (const RendererBackend& backend : backends) {
        comboBackend->addItem(rendererBackendToString(backend),
                QVariant::fromValue(static_cast<int>(backend)));
    }
}

void DlgPrefRenderer::slotUpdate() {
    // Read current config value
    QString currentValue = m_pConfig->getValue(
            ConfigKey(kRendererBackendKey),
            rendererBackendToString(defaultRenderer()));

    RendererBackend current = rendererBackendFromString(currentValue);
    int index = comboBackend->findData(QVariant::fromValue(static_cast<int>(current)));
    if (index >= 0) {
        comboBackend->setCurrentIndex(index);
    }

    // Show platform info
    QString platformInfo;
    QList<RendererBackend> available = availableRenderers();
    QStringList names;
    for (const RendererBackend& b : available) {
        names.append(rendererBackendToString(b));
    }
    platformInfo = QStringLiteral("Available on this platform: %1").arg(names.join(", "));
    if (labelPlatform) {
        labelPlatform->setText(platformInfo);
    }
}

void DlgPrefRenderer::slotApply() {
    int index = comboBackend->currentIndex();
    if (index >= 0) {
        QString backendName = comboBackend->currentText();
        m_pConfig->setValue(ConfigKey(kRendererBackendKey), backendName);
    }
}

void DlgPrefRenderer::slotResetToDefaults() {
    QString defaultValue = rendererBackendToString(defaultRenderer());
    int index = comboBackend->findText(defaultValue);
    if (index >= 0) {
        comboBackend->setCurrentIndex(index);
    }
}

void DlgPrefRenderer::slotSetRendererBackend(int index) {
    Q_UNUSED(index);
    // Value is written on slotApply()
}
