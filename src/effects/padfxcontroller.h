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

/// Pad FX the way rekordbox does them: each of 16 pads (PAD FX 1 = pads 1-8,
/// PAD FX 2 = pads 9-16) holds one effect that sounds while the pad is held.
///
/// Controllers press `[PadFx_ChannelN],padK` (1 = held, 0 = released). Deck 1
/// plays its pad effects through effect unit 3 and deck 2 through unit 4, so
/// the BEAT FX on units 1 and 2 stay untouched. Rolls use the deck's own
/// beatlooproll controls.
///
/// BEAT FX: `[BeatFx],effect` picks the effect type and `[BeatFx],beats`
/// its length; both load into slot 1 of effect units 1 and 2.
///
/// Which effect sits on which pad is user configurable and saved in the
/// `[PadFx]` config group; the catalog below lists what can be assigned.
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
        bool beatInverse = false;
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
