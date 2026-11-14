#pragma once

#include <QHash>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <memory>
#include <vector>

#include "effects/defs.h"
#include "preferences/usersettings.h"

class ControlObject;
class ControlPushButton;
class EffectsManager;

/// Pad FX (rekordbox-style): 16 pads hold effects that sound while held.
/// Deck 1 → unit 3, deck 2 → unit 4 (leaving BEAT FX on units 1-2).
/// BEAT FX (effect + beats) load into unit 1-2 slot 1.
/// Pad assignments are user-configurable in [PadFx] config group.
class PadFxController : public QObject {
    Q_OBJECT
  public:
    static constexpr int kNumPads = 16;
    static constexpr int kNumDecks = 2;

    struct Entry {
        QString key;
        QString name;
        // Built-in effect id, empty for a roll
        QString effectId;
        // Parameter id -> value in the parameter's own units
        QList<QPair<QString, double>> parameters;
        // > 0 for a loop roll of this many beats
        double rollBeats = 0.0;
    };

    PadFxController(UserSettingsPointer pConfig, EffectsManager* pEffectsManager);
    ~PadFxController() override;

    /// All assignable pad effects as {key, name} maps, in menu order.
    QVariantList catalog() const;
    /// Key of the effect on pad 1..16.
    QString padEffect(int pad) const;
    void setPadEffect(int pad, const QString& key);
    void resetToDefaults();
    /// BEAT FX SELECT positions; each holds a user-chosen effect
    static constexpr int kNumBeatFxSlots = 10;
    /// All BEAT FX effects as {key, name} maps.
    QVariantList beatFxCatalog() const;
    /// Key of the effect on BEAT FX position 1..10.
    QString beatFxSlot(int slot) const;
    void setBeatFxSlot(int slot, const QString& key);
    void resetBeatFx();

  private:
    struct BeatFx {
        QString key;
        QString name;
        QString effectId;
        QString beatParam;
        QList<QPair<QString, double>> parameters;
    };

    EffectPresetPointer makePreset(const QString& effectId,
            const QList<QPair<QString, double>>& parameters,
            bool unlink) const;
    void loadBeatFx();
    const BeatFx* beatFxEntry(const QString& key) const;
    void onPad(int deck, int pad, double value);
    const Entry* entry(const QString& key) const;
    void start(int deck, const Entry& fx);
    void stop(int deck);

    UserSettingsPointer m_pConfig;
    EffectsManager* m_pEffectsManager;
    std::vector<Entry> m_catalog;
    std::vector<std::unique_ptr<ControlPushButton>> m_padControls;
    std::vector<BeatFx> m_beatFx;
    // BEAT FX catalog; the first kNumBeatFxSlots are the default order.
    // [BeatFx],effect = selected position, [BeatFx],beats = beat length
    std::unique_ptr<ControlObject> m_pBeatFxEffect;
    std::unique_ptr<ControlObject> m_pBeatFxBeats;
    // [BeatFx],load: (re)load the selected effect
    std::unique_ptr<ControlPushButton> m_pBeatFxLoad;
    // Per deck: what is currently held (roll beats or effect key)
    struct Held {
        int pad = 0;
        double rollBeats = 0.0;
        bool effect = false;
    };
    Held m_held[kNumDecks];
};
