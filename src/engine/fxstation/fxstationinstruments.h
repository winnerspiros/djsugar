#pragma once

#include <QObject>
#include <QString>
#include <QThreadPool>
#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <vector>

#include "preferences/usersettings.h"

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

/// The sounds of the X-Pad, all loaded in the background. Bank 1 plays the
/// app's kick, snare, clap and hi-hat samples; banks 2 to 4 come from
/// Mixxx/FxStation/Bank N on the storage (files named kick, snare, clap and
/// hat, a number prefix like "01_" allowed, or else the first four audio
/// files). An empty Bank 2 plays the bundled DJ Sugar kit. A sound picked
/// in the app for a pad (soundKey) replaces the default of any bank. The
/// engine reads a bank without locking; a bank replaced by a reload is
/// kept until the app quits, since the engine may still be playing it.
class Instruments : public QObject {
    Q_OBJECT
  public:
    explicit Instruments(UserSettingsPointer pConfig, QObject* pParent = nullptr);
    ~Instruments() override;

    /// For the engine thread: the bank, or nullptr while not loaded
    const Bank* bank(int index) const {
        return m_banks[index].load(std::memory_order_acquire);
    }

    /// Loads the banks again, in the background
    void reloadBanks();

    /// The folder of a user bank (1 to 3: Bank 2 to Bank 4)
    static QString bankDirectory(int index);

    /// The sound picked for an instrument of a bank, a file path
    static ConfigKey soundKey(int index, int instrument);

  signals:
    /// A user bank was loaded, with that many sounds
    void bankLoaded(int index, int sounds);

  private:
    void publish(int index, std::unique_ptr<Bank> pBank);
    std::unique_ptr<Bank> loadBank(int index) const;
    /// The sounds of a user bank folder, by name or else in order
    std::array<QString, kInstruments> folderSounds(int index) const;

    const UserSettingsPointer m_pConfig;

    std::array<std::atomic<const Bank*>, kBanks> m_banks;
    std::mutex m_ownedMutex;
    std::vector<std::unique_ptr<Bank>> m_owned;
    QThreadPool m_threadPool;
};

} // namespace fxstation
} // namespace mixxx
