#include "library/samples/dlgsamples.h"

#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QVBoxLayout>

#include "controllers/keyboard/keyboardeventfilter.h"
#include "library/library.h"
#include "library/samples/bundledsamples.h"
#include "library/trackcollectionmanager.h"
#include "moc_dlgsamples.cpp"
#include "track/track.h"
#include "util/assert.h"
#include "widget/wlibrary.h"

DlgSamples::DlgSamples(
        WLibrary* parent,
        UserSettingsPointer pConfig,
        Library* pLibrary,
        KeyboardEventFilter* pKeyboard)
        : QWidget(parent),
          m_pConfig(pConfig),
          m_pLibrary(pLibrary),
          m_pSampleList(new SampleListWidget(this)) {
    QVBoxLayout* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(m_pSampleList);

    m_pSampleList->installEventFilter(pKeyboard);

    connect(m_pSampleList,
            &QListWidget::itemActivated,
            this,
            &DlgSamples::slotSampleActivated);

    refreshBrowseModel();
}

DlgSamples::~DlgSamples() {
}

void DlgSamples::refreshBrowseModel() {
    m_pSampleList->clear();
    m_sampleFiles.clear();
    m_samplesPath = mixxx::samples::bundledSamplesDir(m_pConfig);

    if (m_samplesPath.isEmpty()) {
        qWarning() << "Samples path is empty, no samples to display";
        return;
    }

    const QDir dir(m_samplesPath);
    const QStringList files = mixxx::samples::bundledSampleFiles(m_pConfig);
    for (const QString& path : files) {
        const QString filename = dir.relativeFilePath(path);
        m_sampleFiles.append(filename);
        QListWidgetItem* pItem = new QListWidgetItem(filename);
        pItem->setToolTip(path);
        pItem->setFlags(pItem->flags() | Qt::ItemIsDragEnabled);
        m_pSampleList->addItem(pItem);
    }
}

void DlgSamples::slotSampleActivated(QListWidgetItem* pItem) {
    if (!pItem) {
        return;
    }

    QString filePath = m_samplesPath + pItem->text();
    QFileInfo fi(filePath);
    if (!fi.exists()) {
        qWarning() << "Sample file not found:" << filePath;
        return;
    }

    // Create a Track object for the sample file and load it to Sampler1
    TrackPointer pTrack = Track::newTemporary(filePath);
    if (!pTrack) {
        qWarning() << "Failed to create Track for sample:" << filePath;
        return;
    }

    emit loadTrackToPlayer(pTrack,
            QStringLiteral("[Sampler1]"),
#ifdef __STEM__
            mixxx::StemChannelSelection(),
#endif
            false);
}

void DlgSamples::onSearch(const QString& text) {
    m_currentSearch = text;
    for (int i = 0; i < m_pSampleList->count(); ++i) {
        QListWidgetItem* pItem = m_pSampleList->item(i);
        pItem->setHidden(!text.isEmpty() &&
                !pItem->text().contains(text, Qt::CaseInsensitive));
    }
}

void DlgSamples::slotRestoreSearch() {
    emit restoreSearch(currentSearch());
}

void DlgSamples::saveCurrentViewState() {
    // No persistent state to save for the list view
}

bool DlgSamples::restoreCurrentViewState() {
    return true;
}

bool DlgSamples::hasFocus() const {
    return m_pSampleList->hasFocus();
}

void DlgSamples::setFocus() {
    m_pSampleList->setFocus();
}
