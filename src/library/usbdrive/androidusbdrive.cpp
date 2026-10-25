#include "library/usbdrive/androidusbdrive.h"

#include <libusb.h>

#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QDirIterator>
#include <QElapsedTimer>
#include <QFile>
#include <QFutureWatcher>
#include <QGuiApplication>
#include <QJniEnvironment>
#include <QJniObject>
#include <QPointer>
#include <QStandardPaths>
#include <QtConcurrentRun>
#include <QtCore/private/qandroidextras_p.h>
#include <algorithm>
#include <atomic>
#include <memory>
#include <mutex>
#include <optional>
#include <vector>

#include "controllers/android.h"
#include "library/usbdrive/fatvolume.h"
#include "library/usbdrive/usbmassstorage.h"

namespace mixxx {
namespace usbdrive {

namespace {

// UsbConstants
constexpr jint kClassMassStorage = 8;
constexpr jint kSubclassScsi = 6;
constexpr jint kProtocolBulkOnly = 0x50;
constexpr jint kEndpointBulk = 2;
constexpr jint kDirectionIn = 0x80;

/// mixxx::android::waitForPermission() gives up after 30 s
constexpr qint64 kPermissionTimeoutMs = 29000;

/// The copied tracks are deleted, oldest first, beyond this size
constexpr qint64 kMaxTrackCacheBytes = 4LL * 1024 * 1024 * 1024;

struct Drive {
    QString usbDeviceName;
    QJniObject usbDevice;
    QJniObject connection;
    libusb_context* pContext = nullptr;
    libusb_device_handle* pHandle = nullptr;
    int interfaceNumber = -1;
    std::unique_ptr<UsbMassStorage> pStorage;
    FatVolume volume;
    QString root;
    /// The drive reads one file at a time
    std::mutex mutex;

    ~Drive() {
        if (pHandle) {
            libusb_release_interface(pHandle, interfaceNumber);
            libusb_close(pHandle);
        }
        if (pContext) {
            libusb_exit(pContext);
        }
        if (connection.isValid()) {
            connection.callMethod<void>("close");
        }
    }
};

// Held while looking for drives, which may wait for the USB permission
std::mutex s_scanMutex;
// Guards s_drives, only held briefly
std::mutex s_drivesMutex;
std::vector<std::shared_ptr<Drive>> s_drives;
// The permission is only requested once per plug-in, guarded by s_scanMutex
QStringList s_permissionRequested;
// Set by allowPermissionRequests(), which doesn't wait for s_scanMutex
std::atomic<bool> s_permissionRequestsAllowed = false;

QString cacheRoot() {
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation) +
            QStringLiteral("/usbdrives");
}

QJniObject usbManager() {
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    QJniObject usbService = QJniObject::getStaticObjectField(
            "android/content/Context", "USB_SERVICE", "Ljava/lang/String;");
    return context.callObjectMethod("getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            usbService.object());
}

QList<QJniObject> connectedUsbDevices(const QJniObject& manager) {
    QList<QJniObject> devices;
    QJniObject deviceMap = manager.callObjectMethod("getDeviceList", "()Ljava/util/HashMap;");
    QJniObject values = deviceMap.callObjectMethod("values", "()Ljava/util/Collection;");
    QJniObject array = values.callObjectMethod("toArray", "()[Ljava/lang/Object;");
    QJniEnvironment env;
    const jsize count = array.isValid() ? env->GetArrayLength(array.object<jobjectArray>()) : 0;
    for (jsize i = 0; i < count; ++i) {
        devices.append(QJniObject::fromLocalRef(
                env->GetObjectArrayElement(array.object<jobjectArray>(), i)));
    }
    return devices;
}

QJniObject massStorageInterface(const QJniObject& usbDevice) {
    const jint interfaceCount = usbDevice.callMethod<jint>("getInterfaceCount");
    for (jint i = 0; i < interfaceCount; ++i) {
        QJniObject usbInterface = usbDevice.callObjectMethod("getInterface",
                "(I)Landroid/hardware/usb/UsbInterface;",
                i);
        if (usbInterface.callMethod<jint>("getInterfaceClass") == kClassMassStorage &&
                usbInterface.callMethod<jint>("getInterfaceSubclass") == kSubclassScsi &&
                usbInterface.callMethod<jint>("getInterfaceProtocol") == kProtocolBulkOnly) {
            return usbInterface;
        }
    }
    return {};
}

/// Drives that Android mounted itself are left alone
bool androidMountedRemovableStorage() {
    const QFileInfoList volumes = QDir(QStringLiteral("/storage"))
                                          .entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
    for (const QFileInfo& volume : volumes) {
        if (volume.fileName() != QLatin1String("emulated") &&
                volume.fileName() != QLatin1String("self")) {
            return true;
        }
    }
    return false;
}

QString safeFolderName(QString name) {
    name.replace(QChar('/'), QChar('_'));
    name.replace(QChar('\\'), QChar('_'));
    return name.trimmed().isEmpty() ? QStringLiteral("USB") : name.trimmed();
}

std::shared_ptr<Drive> openDrive(const QJniObject& manager, const QJniObject& usbDevice) {
    auto pDrive = std::make_shared<Drive>();
    pDrive->usbDevice = usbDevice;
    pDrive->usbDeviceName = usbDevice.callMethod<jstring>("getDeviceName").toString();
    const QString productName = usbDevice.callMethod<jstring>("getProductName").toString();

    if (!manager.callMethod<jboolean>("hasPermission",
                "(Landroid/hardware/usb/UsbDevice;)Z",
                usbDevice.object())) {
        if (s_permissionRequested.contains(pDrive->usbDeviceName)) {
            return nullptr;
        }
        // Android may not show the dialog while another app or the system
        // settings are in front, e.g. on the first start. Asked again on the
        // next search for drives.
        if (QGuiApplication::applicationState() != Qt::ApplicationActive) {
            qInfo() << "Not asking for the USB permission for the drive" << productName
                    << "while Mixxx is in the background";
            return nullptr;
        }
        s_permissionRequested.append(pDrive->usbDeviceName);
        qInfo() << "Requesting USB permission for the drive" << productName;
        manager.callMethod<void>("requestPermission",
                "(Landroid/hardware/usb/UsbDevice;Landroid/app/PendingIntent;)V",
                usbDevice.object(),
                mixxx::android::getIntent().object());
        QElapsedTimer waitTimer;
        waitTimer.start();
        if (!mixxx::android::waitForPermission(usbDevice)) {
            if (waitTimer.elapsed() >= kPermissionTimeoutMs) {
                // Unanswered, not denied: asked again on the next search
                qWarning() << "No answer to the USB permission request for the drive"
                           << productName;
                s_permissionRequested.removeAll(pDrive->usbDeviceName);
            } else {
                qWarning() << "USB permission denied for the drive" << productName;
            }
            return nullptr;
        }
        qInfo() << "USB permission granted for the drive" << productName;
    }

    const QJniObject usbInterface = massStorageInterface(usbDevice);
    pDrive->interfaceNumber = usbInterface.callMethod<jint>("getId");
    uint8_t endpointIn = 0;
    uint8_t endpointOut = 0;
    const jint endpointCount = usbInterface.callMethod<jint>("getEndpointCount");
    for (jint i = 0; i < endpointCount; ++i) {
        QJniObject endpoint = usbInterface.callObjectMethod(
                "getEndpoint", "(I)Landroid/hardware/usb/UsbEndpoint;", i);
        if (endpoint.callMethod<jint>("getType") != kEndpointBulk) {
            continue;
        }
        const auto address = static_cast<uint8_t>(endpoint.callMethod<jint>("getAddress"));
        if (endpoint.callMethod<jint>("getDirection") == kDirectionIn) {
            endpointIn = address;
        } else {
            endpointOut = address;
        }
    }
    if (endpointIn == 0 || endpointOut == 0) {
        return nullptr;
    }

    qInfo() << "Opening the USB drive" << productName << "interface"
            << pDrive->interfaceNumber;
    pDrive->connection = manager.callObjectMethod("openDevice",
            "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
            usbDevice.object());
    if (!pDrive->connection.isValid()) {
        qWarning() << "Unable to open the USB drive" << productName;
        return nullptr;
    }
    // Detaches the kernel driver; Android didn't mount the drive anyway
    if (!pDrive->connection.callMethod<jboolean>("claimInterface",
                "(Landroid/hardware/usb/UsbInterface;Z)Z",
                usbInterface.object(),
                jboolean(true))) {
        qWarning() << "Unable to claim the USB drive" << productName;
        return nullptr;
    }
    qInfo() << "Claimed the USB drive" << productName;
    libusb_set_option(nullptr, LIBUSB_OPTION_NO_DEVICE_DISCOVERY);
    if (libusb_init(&pDrive->pContext) < 0) {
        pDrive->pContext = nullptr;
        return nullptr;
    }
    const intptr_t fileDescriptor = pDrive->connection.callMethod<jint>("getFileDescriptor");
    if (libusb_wrap_sys_device(pDrive->pContext, fileDescriptor, &pDrive->pHandle) < 0) {
        pDrive->pHandle = nullptr;
        return nullptr;
    }
    libusb_claim_interface(pDrive->pHandle, pDrive->interfaceNumber);
    qInfo() << "Reading the file system of the USB drive" << productName;

    pDrive->pStorage = std::make_unique<UsbMassStorage>(
            pDrive->pHandle, pDrive->interfaceNumber, endpointIn, endpointOut);
    UsbMassStorage* pStorage = pDrive->pStorage.get();
    if (!pStorage->open() ||
            !pDrive->volume.mount(
                    [pStorage](uint64_t lba, uint32_t count, uint8_t* pDestination) {
                        return pStorage->readBlocks(lba, count, pDestination);
                    },
                    pStorage->blockSize(),
                    pStorage->blockCount())) {
        qInfo() << "No readable FAT drive in" << productName;
        // Give it back to Android, e.g. an empty card reader
        libusb_release_interface(pDrive->pHandle, pDrive->interfaceNumber);
        libusb_attach_kernel_driver(pDrive->pHandle, pDrive->interfaceNumber);
        return nullptr;
    }
    pDrive->root = cacheRoot() + QChar('/') + safeFolderName(pDrive->volume.name());
    QDir().mkpath(pDrive->root);
    qInfo() << "Reading the USB drive" << productName << "as" << pDrive->root;
    return pDrive;
}

std::shared_ptr<Drive> driveFor(const QString& localPath) {
    std::lock_guard lock(s_drivesMutex);
    for (const auto& pDrive : s_drives) {
        if (localPath.startsWith(pDrive->root + QChar('/'))) {
            return pDrive;
        }
    }
    return nullptr;
}

bool isDatabasePath(const QString& relativePath) {
    return relativePath.startsWith(QLatin1String("PIONEER/"), Qt::CaseInsensitive) ||
            relativePath.startsWith(QLatin1String(".PIONEER/"), Qt::CaseInsensitive) ||
            relativePath.startsWith(QLatin1String("Engine Library/"), Qt::CaseInsensitive);
}

void trimTrackCache(const QString& keep) {
    struct CachedFile {
        QString path;
        qint64 size;
        QDateTime used;
    };
    std::vector<CachedFile> files;
    qint64 total = 0;
    QDirIterator it(cacheRoot(), QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QFileInfo info(it.next());
        const QString relative = QDir(cacheRoot()).relativeFilePath(info.filePath());
        // <drive>/<path on the drive>
        if (isDatabasePath(relative.section(QChar('/'), 1))) {
            continue;
        }
        files.push_back({info.filePath(), info.size(), info.lastModified()});
        total += info.size();
    }
    std::sort(files.begin(), files.end(), [](const auto& a, const auto& b) {
        return a.used < b.used;
    });
    for (const auto& file : files) {
        if (total <= kMaxTrackCacheBytes) {
            break;
        }
        if (file.path != keep && QFile::remove(file.path)) {
            total -= file.size;
        }
    }
}

/// Copies a file of the drive unless the local copy is current
bool ensureLocal(const std::shared_ptr<Drive>& pDrive, const QString& relativePath) {
    const std::lock_guard lock(pDrive->mutex);
    const std::optional<FatVolume::Entry> entry = pDrive->volume.find(relativePath);
    if (!entry || entry->isDirectory) {
        return false;
    }
    const QString localPath = pDrive->root + QChar('/') + relativePath;
    QFileInfo local(localPath);
    const bool isDatabase = isDatabasePath(relativePath);
    if (local.exists() && local.size() == entry->size &&
            (!isDatabase || local.lastModified() == entry->modified)) {
        if (!isDatabase) {
            // Recently used tracks are kept in the cache
            QFile file(localPath);
            if (file.open(QIODevice::ReadWrite)) {
                file.setFileTime(QDateTime::currentDateTime(), QFileDevice::FileModificationTime);
            }
        }
        return true;
    }
    QElapsedTimer timer;
    timer.start();
    if (!pDrive->volume.copyFile(*entry, localPath)) {
        return false;
    }
    if (isDatabase && entry->modified.isValid()) {
        QFile file(localPath);
        if (file.open(QIODevice::ReadWrite)) {
            file.setFileTime(entry->modified, QFileDevice::FileModificationTime);
        }
    }
    qInfo() << "Copied" << relativePath << "from the USB drive," << entry->size / 1024 << "KiB in"
            << timer.elapsed() << "ms";
    if (!isDatabase) {
        trimTrackCache(localPath);
    }
    return true;
}

} // namespace

QFileInfoList driveRoots() {
    std::lock_guard scanLock(s_scanMutex);
    std::vector<std::shared_ptr<Drive>> drives;
    {
        std::lock_guard lock(s_drivesMutex);
        drives = s_drives;
    }
    QJniObject manager = usbManager();
    if (!manager.isValid()) {
        return {};
    }
    const QList<QJniObject> usbDevices = connectedUsbDevices(manager);
    QStringList connected;
    for (const QJniObject& usbDevice : usbDevices) {
        if (massStorageInterface(usbDevice).isValid()) {
            connected.append(usbDevice.callMethod<jstring>("getDeviceName").toString());
        }
    }
    // A drive that stopped answering was reset or plugged again (e.g. by a
    // hub), and Android dropped its permission: ask again
    for (const auto& pDrive : drives) {
        if (pDrive->pStorage->isDisconnected()) {
            qInfo() << "The USB drive" << pDrive->usbDeviceName << "was disconnected";
            s_permissionRequested.erase(std::remove(s_permissionRequested.begin(),
                                                s_permissionRequested.end(),
                                                pDrive->usbDeviceName),
                    s_permissionRequested.end());
        }
    }
    // Forget the drives that are gone
    drives.erase(std::remove_if(drives.begin(),
                           drives.end(),
                           [&connected](const auto& pDrive) {
                               return !connected.contains(pDrive->usbDeviceName) ||
                                       pDrive->pStorage->isDisconnected();
                           }),
            drives.end());
    s_permissionRequested.erase(std::remove_if(s_permissionRequested.begin(),
                                        s_permissionRequested.end(),
                                        [&connected](const QString& name) {
                                            return !connected.contains(name);
                                        }),
            s_permissionRequested.end());
    if (s_permissionRequestsAllowed.exchange(false)) {
        s_permissionRequested.clear();
    }

    if (!androidMountedRemovableStorage()) {
        for (const QJniObject& usbDevice : usbDevices) {
            if (!massStorageInterface(usbDevice).isValid()) {
                continue;
            }
            const QString name = usbDevice.callMethod<jstring>("getDeviceName").toString();
            const bool known = std::any_of(drives.cbegin(),
                    drives.cend(),
                    [&name](const auto& pDrive) { return pDrive->usbDeviceName == name; });
            if (known) {
                continue;
            }
            if (auto pDrive = openDrive(manager, usbDevice)) {
                drives.push_back(pDrive);
            }
        }
    }

    {
        std::lock_guard lock(s_drivesMutex);
        s_drives = drives;
    }
    QFileInfoList roots;
    for (const auto& pDrive : drives) {
        roots.append(QFileInfo(pDrive->root));
    }
    return roots;
}

void prepareFiles(const QString& root, const QStringList& relativePaths) {
    const auto pDrive = driveFor(root + QStringLiteral("/"));
    if (!pDrive) {
        return;
    }
    for (const QString& relativePath : relativePaths) {
        if (relativePath.endsWith(QLatin1String("/*.db"))) {
            const QString folder = relativePath.chopped(5);
            const QList<FatVolume::Entry> entries = pDrive->volume.list(folder);
            for (const auto& entry : entries) {
                if (!entry.isDirectory && entry.name.endsWith(QLatin1String(".db"), Qt::CaseInsensitive)) {
                    ensureLocal(pDrive, folder + QChar('/') + entry.name);
                }
            }
        } else {
            ensureLocal(pDrive, relativePath);
        }
    }
}

void allowPermissionRequests() {
    s_permissionRequestsAllowed = true;
}

bool fetchFile(const QString& localPath) {
    const auto pDrive = driveFor(localPath);
    if (!pDrive) {
        return QFileInfo::exists(localPath);
    }
    return ensureLocal(pDrive, localPath.mid(pDrive->root.size() + 1));
}

QString driveSerial(const QString& root) {
    const auto pDrive = driveFor(root + QStringLiteral("/"));
    return pDrive ? pDrive->volume.serial() : QString();
}

std::optional<QList<DriveFile>> listFiles(const QString& root, bool* pComplete) {
    const auto pDrive = driveFor(root + QStringLiteral("/"));
    if (!pDrive) {
        return std::nullopt;
    }
    const int readErrors = pDrive->volume.readErrors();
    // Reads only the directories, the file system guards itself
    QList<DriveFile> files;
    // The root first
    QStringList pending{QString()};
    while (!pending.isEmpty()) {
        const QString folder = pending.takeFirst();
        const QList<FatVolume::Entry> entries = pDrive->volume.list(folder);
        for (const auto& entry : entries) {
            const QString path = folder.isEmpty() ? entry.name : folder + QChar('/') + entry.name;
            if (entry.isDirectory) {
                pending.append(path);
            } else {
                files.append({path, static_cast<qint64>(entry.size), entry.modified});
            }
        }
    }
    *pComplete = pDrive->volume.readErrors() == readErrors &&
            !pDrive->pStorage->isDisconnected();
    return files;
}

bool copyFileTo(const QString& root, const QString& relativePath, const QString& destination) {
    const auto pDrive = driveFor(root + QStringLiteral("/"));
    if (!pDrive) {
        return false;
    }
    const std::lock_guard lock(pDrive->mutex);
    const std::optional<FatVolume::Entry> entry = pDrive->volume.find(relativePath);
    return entry && pDrive->volume.copyFile(*entry, destination);
}

bool isOnDrive(const QString& localPath) {
    return driveFor(localPath) != nullptr;
}

void fetchTrack(const QString& localPath, QObject* pContext, std::function<void()> then) {
    const auto pDrive = driveFor(localPath);
    if (!pDrive) {
        then();
        return;
    }
    const QString relativePath = localPath.mid(pDrive->root.size() + 1);
    auto* pWatcher = new QFutureWatcher<bool>(pContext);
    QObject::connect(pWatcher,
            &QFutureWatcher<bool>::finished,
            pContext,
            [pWatcher, then = std::move(then), localPath]() {
                if (pWatcher->result()) {
                    then();
                } else {
                    qWarning() << "Unable to copy" << localPath << "from the USB drive";
                }
                pWatcher->deleteLater();
            });
    pWatcher->setFuture(QtConcurrent::run([pDrive, relativePath]() {
        return ensureLocal(pDrive, relativePath);
    }));
}

} // namespace usbdrive
} // namespace mixxx
