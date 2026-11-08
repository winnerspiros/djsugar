#pragma once

#include <QByteArray>
#include <QDateTime>
#include <QHash>
#include <QList>
#include <QString>
#include <atomic>
#include <cstdint>
#include <functional>
#include <mutex>
#include <optional>

namespace mixxx {
namespace usbdrive {

/// Read-only FAT16/FAT32 file system on a block device. Handles drives with
/// an MBR or GPT partition table and drives formatted without one
/// ("superfloppy"), which e.g. Pioneer players accept but many Android
/// devices don't mount.
class FatVolume {
  public:
    /// Reads `count` blocks of the device starting at `lba`
    using BlockReader = std::function<bool(uint64_t lba, uint32_t count, uint8_t* pDestination)>;

    struct Entry {
        QString name;
        bool isDirectory = false;
        uint32_t firstCluster = 0;
        uint32_t size = 0;
        QDateTime modified;
    };

    /// Returns false if the device holds no supported file system
    bool mount(BlockReader reader, uint32_t blockSize, uint64_t blockCount);

    /// The volume label, or the serial number if there is none
    QString name() const;
    /// The serial number as Android shows it, e.g. "1A2B-3C4D"
    const QString& serial() const {
        return m_serial;
    }

    /// Looks up a path relative to the root, with '/' separators. The names
    /// are compared case-insensitively, like Windows does.
    std::optional<Entry> find(const QString& path);
    QList<Entry> list(const QString& path);

    /// How many reads of directories or of the FAT failed so far. A
    /// directory that failed to read looks like an empty or shorter one.
    int readErrors() const {
        return m_readErrors;
    }

    /// Copies a file to the local file system. The destination is only
    /// replaced once the copy is complete.
    bool copyFile(const Entry& entry, const QString& destination);

  private:
    bool readSectors(uint64_t sector, uint32_t count, uint8_t* pDestination);
    bool mountAt(uint64_t firstBlock);
    uint32_t nextCluster(uint32_t cluster);
    uint64_t clusterSector(uint32_t cluster) const;
    /// Sets *pComplete to false if a part could not be read
    QList<Entry> readDirectory(uint32_t firstCluster, bool* pComplete);
    QList<Entry> directory(uint32_t firstCluster);

    BlockReader m_readBlocks;
    uint32_t m_blockSize = 0;
    uint64_t m_blockCount = 0;
    uint64_t m_firstBlock = 0;

    bool m_isFat32 = false;
    uint32_t m_bytesPerSector = 0;
    uint32_t m_sectorsPerCluster = 0;
    uint32_t m_fatStart = 0;
    uint32_t m_rootDirStart = 0;
    uint32_t m_rootDirSectors = 0;
    uint32_t m_rootCluster = 0;
    uint64_t m_dataStart = 0;
    uint32_t m_clusterCount = 0;
    QString m_label;
    QString m_serial;

    std::mutex m_mutex;
    QHash<uint32_t, QByteArray> m_fatCache;
    QHash<uint32_t, QList<Entry>> m_directoryCache;
    std::atomic<int> m_readErrors = 0;
};

} // namespace usbdrive
} // namespace mixxx
