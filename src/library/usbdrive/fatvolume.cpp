#include "library/usbdrive/fatvolume.h"

#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <algorithm>
#include <cstring>
#include <vector>

namespace mixxx {
namespace usbdrive {

namespace {

constexpr uint8_t kAttributeVolumeId = 0x08;
constexpr uint8_t kAttributeDirectory = 0x10;
constexpr uint8_t kAttributeLongName = 0x0F;
constexpr int kEntrySize = 32;
// Directories are cached, the FAT only partially
constexpr int kMaxCachedFatSectors = 4096;
// Size of the reads when copying files
constexpr uint32_t kMaxCopyBytes = 1024 * 1024;

uint16_t le16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0] | (p[1] << 8));
}

uint32_t le32(const uint8_t* p) {
    return p[0] | (p[1] << 8) | (p[2] << 16) | (static_cast<uint32_t>(p[3]) << 24);
}

uint64_t le64(const uint8_t* p) {
    return le32(p) | (static_cast<uint64_t>(le32(p + 4)) << 32);
}

bool isPowerOfTwo(uint32_t value) {
    return value != 0 && (value & (value - 1)) == 0;
}

/// Checks the BIOS parameter block of a FAT boot sector
bool isFatBootSector(const uint8_t* p) {
    const uint32_t bytesPerSector = le16(p + 11);
    return (p[0] == 0xEB || p[0] == 0xE9) && p[510] == 0x55 && p[511] == 0xAA &&
            bytesPerSector >= 512 && bytesPerSector <= 4096 && isPowerOfTwo(bytesPerSector) &&
            isPowerOfTwo(p[13]) && le16(p + 14) > 0 && (p[16] == 1 || p[16] == 2);
}

QString trimmedLatin1(const uint8_t* p, int length) {
    return QString::fromLatin1(reinterpret_cast<const char*>(p), length).trimmed();
}

QDateTime fatDateTime(uint16_t date, uint16_t time) {
    const QDate day(1980 + (date >> 9), (date >> 5) & 0x0F, date & 0x1F);
    const QTime clock(time >> 11, (time >> 5) & 0x3F, (time & 0x1F) * 2);
    return QDateTime(day, clock);
}

uint8_t shortNameChecksum(const uint8_t* p) {
    uint8_t sum = 0;
    for (int i = 0; i < 11; ++i) {
        sum = static_cast<uint8_t>(((sum & 1) ? 0x80 : 0) + (sum >> 1) + p[i]);
    }
    return sum;
}

} // namespace

bool FatVolume::mount(BlockReader reader, uint32_t blockSize, uint64_t blockCount) {
    m_readBlocks = std::move(reader);
    m_blockSize = blockSize;
    m_blockCount = blockCount;

    std::vector<uint8_t> block(blockSize);
    if (!m_readBlocks(0, 1, block.data())) {
        return false;
    }
    if (std::memcmp(block.data() + 3, "EXFAT   ", 8) == 0) {
        qWarning() << "USB drive: exFAT is not supported yet";
        return false;
    }
    // Formatted without a partition table
    if (isFatBootSector(block.data()) && mountAt(0)) {
        return true;
    }
    if (block[510] != 0x55 || block[511] != 0xAA) {
        return false;
    }
    std::vector<uint64_t> partitions;
    bool isGpt = false;
    for (int i = 0; i < 4; ++i) {
        const uint8_t* pEntry = block.data() + 446 + i * 16;
        if (pEntry[4] == 0xEE) {
            isGpt = true;
        } else if (pEntry[4] != 0 && le32(pEntry + 8) != 0) {
            partitions.push_back(le32(pEntry + 8));
        }
    }
    if (isGpt) {
        std::vector<uint8_t> header(blockSize);
        if (m_readBlocks(1, 1, header.data()) && std::memcmp(header.data(), "EFI PART", 8) == 0) {
            const uint64_t entriesLba = le64(header.data() + 72);
            const uint32_t entryCount = std::min<uint32_t>(le32(header.data() + 80), 128);
            const uint32_t entrySize = le32(header.data() + 84);
            if (entrySize >= 128 && entrySize <= 1024) {
                const uint32_t blocks = (entryCount * entrySize + blockSize - 1) / blockSize;
                std::vector<uint8_t> entries(static_cast<size_t>(blocks) * blockSize);
                if (m_readBlocks(entriesLba, blocks, entries.data())) {
                    for (uint32_t i = 0; i < entryCount; ++i) {
                        const uint64_t firstLba = le64(entries.data() + i * entrySize + 32);
                        if (firstLba != 0) {
                            partitions.push_back(firstLba);
                        }
                    }
                }
            }
        }
    }
    for (uint64_t firstBlock : partitions) {
        if (firstBlock < blockCount && m_readBlocks(firstBlock, 1, block.data()) &&
                isFatBootSector(block.data()) && mountAt(firstBlock)) {
            return true;
        }
    }
    return false;
}

bool FatVolume::mountAt(uint64_t firstBlock) {
    m_firstBlock = firstBlock;
    std::vector<uint8_t> boot(m_blockSize);
    if (!m_readBlocks(firstBlock, 1, boot.data())) {
        return false;
    }
    const uint8_t* p = boot.data();
    m_bytesPerSector = le16(p + 11);
    if (m_bytesPerSector < m_blockSize || m_bytesPerSector % m_blockSize != 0) {
        return false;
    }
    m_sectorsPerCluster = p[13];
    const uint32_t reservedSectors = le16(p + 14);
    const uint32_t fatCount = p[16];
    const uint32_t rootEntries = le16(p + 17);
    const uint32_t totalSectors = le16(p + 19) != 0 ? le16(p + 19) : le32(p + 32);
    const uint32_t fatSize = le16(p + 22) != 0 ? le16(p + 22) : le32(p + 36);
    m_rootDirSectors = (rootEntries * kEntrySize + m_bytesPerSector - 1) / m_bytesPerSector;
    m_fatStart = reservedSectors;
    m_rootDirStart = reservedSectors + fatCount * fatSize;
    m_dataStart = m_rootDirStart + m_rootDirSectors;
    if (fatSize == 0 || totalSectors <= m_dataStart) {
        return false;
    }
    m_clusterCount = static_cast<uint32_t>((totalSectors - m_dataStart) / m_sectorsPerCluster);
    if (m_clusterCount < 4085) {
        qWarning() << "USB drive: FAT12 is not supported";
        return false;
    }
    m_isFat32 = m_clusterCount >= 65525;
    const uint8_t* pExtended = p + (m_isFat32 ? 64 : 36);
    m_rootCluster = m_isFat32 ? le32(p + 44) : 0;
    const uint32_t serial = le32(pExtended + 3);
    m_serial = QStringLiteral("%1-%2")
                       .arg(serial >> 16, 4, 16, QChar('0'))
                       .arg(serial & 0xFFFF, 4, 16, QChar('0'))
                       .toUpper();
    m_label = trimmedLatin1(pExtended + 7, 11);
    if (m_label == QLatin1String("NO NAME")) {
        m_label.clear();
    }
    m_fatCache.clear();
    m_directoryCache.clear();

    // The label in the root directory is the one the systems show
    const QList<Entry> root = directory(m_rootCluster);
    qInfo() << "USB drive: FAT" << (m_isFat32 ? 32 : 16) << "volume" << name() << m_serial
            << "at block" << firstBlock << "with" << root.size() << "root entries";
    return true;
}

QString FatVolume::name() const {
    return m_label.isEmpty() ? m_serial : m_label;
}

bool FatVolume::readSectors(uint64_t sector, uint32_t count, uint8_t* pDestination) {
    const uint32_t blocksPerSector = m_bytesPerSector / m_blockSize;
    return m_readBlocks(m_firstBlock + sector * blocksPerSector,
            count * blocksPerSector,
            pDestination);
}

uint64_t FatVolume::clusterSector(uint32_t cluster) const {
    return m_dataStart + static_cast<uint64_t>(cluster - 2) * m_sectorsPerCluster;
}

uint32_t FatVolume::nextCluster(uint32_t cluster) {
    const uint32_t offset = m_isFat32 ? cluster * 4 : cluster * 2;
    const uint32_t fatSector = m_fatStart + offset / m_bytesPerSector;
    const uint32_t offsetInSector = offset % m_bytesPerSector;
    auto it = m_fatCache.constFind(fatSector);
    if (it == m_fatCache.constEnd()) {
        QByteArray sector(m_bytesPerSector, Qt::Uninitialized);
        if (!readSectors(fatSector, 1, reinterpret_cast<uint8_t*>(sector.data()))) {
            ++m_readErrors;
            return 0;
        }
        if (m_fatCache.size() >= kMaxCachedFatSectors) {
            m_fatCache.clear();
        }
        it = m_fatCache.insert(fatSector, sector);
    }
    const auto* p = reinterpret_cast<const uint8_t*>(it->constData()) + offsetInSector;
    const uint32_t next = m_isFat32 ? (le32(p) & 0x0FFFFFFF) : le16(p);
    const uint32_t endOfChain = m_isFat32 ? 0x0FFFFFF8 : 0xFFF8;
    if (next < 2 || next >= endOfChain || next >= m_clusterCount + 2) {
        return 0;
    }
    return next;
}

QList<FatVolume::Entry> FatVolume::directory(uint32_t firstCluster) {
    auto it = m_directoryCache.constFind(firstCluster);
    if (it != m_directoryCache.constEnd()) {
        return it.value();
    }
    bool complete = true;
    const QList<Entry> entries = readDirectory(firstCluster, &complete);
    if (complete) {
        m_directoryCache.insert(firstCluster, entries);
    } else {
        // Read again next time
        ++m_readErrors;
    }
    return entries;
}

QList<FatVolume::Entry> FatVolume::readDirectory(uint32_t firstCluster, bool* pComplete) {
    QList<Entry> entries;
    std::vector<uint8_t> buffer;
    const uint32_t clusterBytes = m_sectorsPerCluster * m_bytesPerSector;

    std::vector<char16_t> longName;
    uint8_t longNameChecksum = 0;
    bool done = false;

    auto parse = [&](const uint8_t* pData, uint32_t bytes) {
        for (uint32_t offset = 0; offset + kEntrySize <= bytes && !done; offset += kEntrySize) {
            const uint8_t* p = pData + offset;
            if (p[0] == 0x00) {
                done = true;
                break;
            }
            if (p[0] == 0xE5) {
                longName.clear();
                continue;
            }
            const uint8_t attributes = p[11];
            if (attributes == kAttributeLongName) {
                const int sequence = p[0] & 0x1F;
                if (sequence == 0) {
                    longName.clear();
                    continue;
                }
                if (p[0] & 0x40) {
                    longName.assign(static_cast<size_t>(sequence) * 13, 0xFFFF);
                    longNameChecksum = p[13];
                }
                const size_t base = static_cast<size_t>(sequence - 1) * 13;
                if (base + 13 > longName.size()) {
                    longName.clear();
                    continue;
                }
                static const int kCharOffsets[13] = {1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30};
                for (int i = 0; i < 13; ++i) {
                    longName[base + i] = static_cast<char16_t>(le16(p + kCharOffsets[i]));
                }
                continue;
            }
            if (attributes & kAttributeVolumeId) {
                if (firstCluster == m_rootCluster) {
                    const QString label = trimmedLatin1(p, 11);
                    if (!label.isEmpty()) {
                        m_label = label;
                    }
                }
                longName.clear();
                continue;
            }
            Entry entry;
            if (!longName.empty() && longNameChecksum == shortNameChecksum(p)) {
                size_t length = 0;
                while (length < longName.size() && longName[length] != 0 &&
                        longName[length] != 0xFFFF) {
                    ++length;
                }
                entry.name = QString::fromUtf16(longName.data(), static_cast<qsizetype>(length));
            } else {
                QByteArray shortName(reinterpret_cast<const char*>(p), 11);
                if (static_cast<uint8_t>(shortName[0]) == 0x05) {
                    shortName[0] = static_cast<char>(0xE5);
                }
                QString base = QString::fromLatin1(shortName.left(8)).trimmed();
                QString extension = QString::fromLatin1(shortName.mid(8, 3)).trimmed();
                // Windows stores all-lowercase names as short names with flags
                if (p[12] & 0x08) {
                    base = base.toLower();
                }
                if (p[12] & 0x10) {
                    extension = extension.toLower();
                }
                entry.name = extension.isEmpty() ? base : base + QChar('.') + extension;
            }
            longName.clear();
            if (entry.name == QLatin1String(".") || entry.name == QLatin1String("..")) {
                continue;
            }
            entry.isDirectory = attributes & kAttributeDirectory;
            entry.firstCluster = (m_isFat32 ? (static_cast<uint32_t>(le16(p + 20)) << 16) : 0) |
                    le16(p + 26);
            entry.size = le32(p + 28);
            entry.modified = fatDateTime(le16(p + 24), le16(p + 22));
            entries.append(entry);
        }
    };

    if (firstCluster == 0 && !m_isFat32) {
        // The FAT16 root directory has a fixed location
        buffer.resize(static_cast<size_t>(m_rootDirSectors) * m_bytesPerSector);
        if (readSectors(m_rootDirStart, m_rootDirSectors, buffer.data())) {
            parse(buffer.data(), static_cast<uint32_t>(buffer.size()));
        } else {
            *pComplete = false;
        }
        return entries;
    }
    buffer.resize(clusterBytes);
    uint32_t cluster = firstCluster;
    // Guards against loops in a damaged FAT
    uint32_t remaining = m_clusterCount;
    while (cluster >= 2 && !done && remaining-- > 0) {
        if (!readSectors(clusterSector(cluster), m_sectorsPerCluster, buffer.data())) {
            *pComplete = false;
            break;
        }
        parse(buffer.data(), clusterBytes);
        cluster = nextCluster(cluster);
    }
    return entries;
}

std::optional<FatVolume::Entry> FatVolume::find(const QString& path) {
    std::lock_guard lock(m_mutex);
    const QStringList parts = path.split(QChar('/'), Qt::SkipEmptyParts);
    Entry current;
    current.isDirectory = true;
    current.firstCluster = m_rootCluster;
    for (const QString& part : parts) {
        if (!current.isDirectory) {
            return std::nullopt;
        }
        const QList<Entry> entries = directory(current.firstCluster);
        auto it = std::find_if(entries.cbegin(), entries.cend(), [&part](const Entry& entry) {
            return entry.name.compare(part, Qt::CaseInsensitive) == 0;
        });
        if (it == entries.cend()) {
            return std::nullopt;
        }
        current = *it;
    }
    return current;
}

QList<FatVolume::Entry> FatVolume::list(const QString& path) {
    const std::optional<Entry> entry = find(path);
    if (!entry || !entry->isDirectory) {
        return {};
    }
    std::lock_guard lock(m_mutex);
    return directory(entry->firstCluster);
}

bool FatVolume::copyFile(const Entry& entry, const QString& destination) {
    if (entry.isDirectory) {
        return false;
    }
    QDir().mkpath(QFileInfo(destination).absolutePath());
    const QString partialPath = destination + QStringLiteral(".part");
    QFile file(partialPath);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        qWarning() << "USB drive: unable to write" << partialPath;
        return false;
    }
    const uint32_t clusterBytes = m_sectorsPerCluster * m_bytesPerSector;
    const uint32_t maxRunClusters = std::max<uint32_t>(1, kMaxCopyBytes / clusterBytes);
    std::vector<uint8_t> buffer(static_cast<size_t>(maxRunClusters) * clusterBytes);
    uint32_t remainingBytes = entry.size;
    uint32_t cluster = entry.firstCluster;
    bool ok = true;
    while (remainingBytes > 0) {
        if (cluster < 2) {
            ok = false;
            break;
        }
        // Read runs of consecutive clusters at once
        uint32_t runLength = 1;
        uint32_t next;
        {
            std::lock_guard lock(m_mutex);
            next = nextCluster(cluster);
            while (next == cluster + runLength && runLength < maxRunClusters &&
                    static_cast<uint64_t>(runLength) * clusterBytes < remainingBytes) {
                ++runLength;
                next = nextCluster(next);
            }
        }
        if (!readSectors(clusterSector(cluster), runLength * m_sectorsPerCluster, buffer.data())) {
            ok = false;
            break;
        }
        const uint32_t bytes = std::min<uint32_t>(remainingBytes, runLength * clusterBytes);
        if (file.write(reinterpret_cast<const char*>(buffer.data()), bytes) != bytes) {
            ok = false;
            break;
        }
        remainingBytes -= bytes;
        cluster = next;
    }
    file.close();
    if (!ok) {
        QFile::remove(partialPath);
        qWarning() << "USB drive: copying to" << destination << "failed";
        return false;
    }
    QFile::remove(destination);
    if (!QFile::rename(partialPath, destination)) {
        QFile::remove(partialPath);
        return false;
    }
    return true;
}

} // namespace usbdrive
} // namespace mixxx
