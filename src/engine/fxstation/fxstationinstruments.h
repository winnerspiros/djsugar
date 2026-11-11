#pragma once

#include <QObject>
#include <QString>
#include <QThreadPool>
#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <vector>

namespace mixxx {
namespace fxstation {

/// Kick, snare, clap and hi-hat
constexpr int kInstruments = 4;
/// Bank 1 has the sounds of the app, banks 2 to 4 the user's
constexpr int kBanks = 4;

/// A sound of the X-Pad: interleaved stereo samples
struct Instrument {
    std::vector<float> samples;
    float sampleRate = 48000.f;
    int frames() const {
        return static_cast<int>(samples.size() / 2);
    }
};

struct Bank {
    std::array<Instrument, kInstruments> instruments;
};

/// The sounds of the X-Pad. Bank 1 is synthesized (no sample files, no
/// rights to care about); banks 2 to 4 are loaded in the background from
/// Mixxx/FxStation/Bank N on the storage (files whose names start with
/// kick, snare, clap and hat, or else the first four audio files). The
/// engine reads a bank without locking; a bank replaced by a reload is
/// kept until the app quits, since the engine may still be playing it.
class Instruments : public QObject {
    Q_OBJECT
  public:
    explicit Instruments(QObject* pParent = nullptr);
    ~Instruments() override;

    /// For the engine thread: the bank, or nullptr while not loaded
    const Bank* bank(int index) const {
        return m_banks[index].load(std::memory_order_acquire);
    }

    /// Loads the user banks again, in the background
    void reloadUserBanks();

    /// The folder of a user bank (1 to 3: Bank 2 to Bank 4)
    static QString bankDirectory(int index);

  signals:
    /// A user bank was loaded, with that many sounds
    void bankLoaded(int index, int sounds);

  private:
    void publish(int index, std::unique_ptr<Bank> pBank);
    static std::unique_ptr<Bank> loadBank(int index);

    std::array<std::atomic<const Bank*>, kBanks> m_banks;
    std::mutex m_ownedMutex;
    std::vector<std::unique_ptr<Bank>> m_owned;
    QThreadPool m_threadPool;
};

} // namespace fxstation
} // namespace mixxx
