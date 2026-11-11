#include "effects/padfxcontroller.h"

#include <QDomDocument>
#include <algorithm>

#include "control/controlobject.h"
#include "control/controlpushbutton.h"
#include "effects/backends/builtin/beatdivisions.h"
#include "effects/backends/effectsbackendmanager.h"
#include "effects/effectchain.h"
#include "effects/effectchainmixmode.h"
#include "effects/effectslot.h"
#include "effects/effectsmanager.h"
#include "effects/presets/effectpreset.h"
#include "effects/presets/effectpresetmanager.h"
#include "effects/presets/effectxmlelements.h"
#include "moc_padfxcontroller.cpp"

namespace {

const QString kConfigGroup = QStringLiteral("[PadFx]");
// Saved BEAT FX order, separate from the [BeatFx] controls
const QString kBeatFxConfigGroup = QStringLiteral("[BeatFxSlots]");

// Effect unit that plays the pad effects of deck 1 and deck 2.
constexpr int kUnitForDeck[PadFxController::kNumDecks] = {3, 4};

// rekordbox-style default layout. PAD FX 1 (pads 1-8): echoes, reverb,
// trans, filter. PAD FX 2 (pads 9-16): rolls, modulation, crush, tape stop.
const char* const kDefaultPads[PadFxController::kNumPads] = {
        "echo_1_2",
        "echo_3_4",
        "echo_1",
        "spiral_1_2",
        "reverb",
        "trans_1_4",
        "trans_1_2",
        "hpf",
        "roll_1_16",
        "roll_1_8",
        "roll_1_4",
        "roll_1_2",
        "flanger",
        "phaser",
        "crush",
        "tapestop",
};

QString channelGroup(int deck) {
    return QStringLiteral("[Channel%1]").arg(deck + 1);
}

QString unitGroup(int deck) {
    return QStringLiteral("[EffectRack1_EffectUnit%1]").arg(kUnitForDeck[deck]);
}

QString rollControl(double beats) {
    return QStringLiteral("beatlooproll_%1_activate").arg(beats);
}

} // namespace

PadFxController::PadFxController(
        UserSettingsPointer pConfig, EffectsManager* pEffectsManager)
        : m_pConfig(pConfig),
          m_pEffectsManager(pEffectsManager) {
    const QString djEcho = QStringLiteral("org.mixxx.effects.djecho");
    const QString tremolo = QStringLiteral("org.mixxx.effects.tremolo");
    const QString filter = QStringLiteral("org.mixxx.effects.filter");
    const QString reverb = QStringLiteral("org.mixxx.effects.reverb");
    const QString flanger = QStringLiteral("org.mixxx.effects.flanger");
    // Tempo-synced parameters are beat-division selectors; presets store
    // the step index.
    const auto div = [](double beats) {
        return static_cast<double>(djfx::divisionIndex(beats));
    };
    m_catalog = {
            {"echo_1_4",
                    tr("Echo 1/4"),
                    djEcho,
                    {{"delay_time", div(0.25)}, {"feedback", 0.6}, {"amount", 1.0}}},
            {"echo_1_2",
                    tr("Echo 1/2"),
                    djEcho,
                    {{"delay_time", div(0.5)}, {"feedback", 0.6}, {"amount", 1.0}}},
            {"echo_3_4",
                    tr("Echo 3/4"),
                    djEcho,
                    {{"delay_time", div(0.75)}, {"feedback", 0.6}, {"amount", 1.0}}},
            {"echo_1",
                    tr("Echo 1"),
                    djEcho,
                    {{"delay_time", div(1.0)}, {"feedback", 0.65}, {"amount", 1.0}}},
            {"echo_2",
                    tr("Echo 2"),
                    djEcho,
                    {{"delay_time", div(2.0)}, {"feedback", 0.7}, {"amount", 1.0}}},
            {"spiral_1_2",
                    tr("Spiral 1/2"),
                    QStringLiteral("org.mixxx.effects.spiral"),
                    {{"delay_time", div(0.5)}, {"feedback", 0.7}, {"amount", 1.0}}},
            {"reverb",
                    tr("Reverb"),
                    reverb,
                    {{"send_amount", 0.8}, {"size", 0.85}, {"decay", 4.0}}},
            {"trans_1_4",
                    tr("Trans 1/4"),
                    tremolo,
                    {{"rate", div(0.25)}, {"depth", 1.0}, {"shape", 0.0}}},
            {"trans_1_2",
                    tr("Trans 1/2"),
                    tremolo,
                    {{"rate", div(0.5)}, {"depth", 1.0}, {"shape", 0.0}}},
            {"trans_1",
                    tr("Trans 1"),
                    tremolo,
                    {{"rate", div(1.0)}, {"depth", 1.0}, {"shape", 0.0}}},
            // Filter knob: left of center low-pass, right high-pass.
            {"hpf",
                    tr("High-pass filter"),
                    filter,
                    {{"filter", 0.8}, {"q", 0.6}}},
            {"lpf",
                    tr("Low-pass filter"),
                    filter,
                    {{"filter", 0.22}, {"q", 0.6}}},
            {"roll_1_16", tr("Roll 1/16"), QString(), {}, 1.0 / 16},
            {"roll_1_8", tr("Roll 1/8"), QString(), {}, 1.0 / 8},
            {"roll_1_4", tr("Roll 1/4"), QString(), {}, 0.25},
            {"roll_1_2", tr("Roll 1/2"), QString(), {}, 0.5},
            {"roll_1", tr("Roll 1"), QString(), {}, 1.0},
            {"roll_2", tr("Roll 2"), QString(), {}, 2.0},
            {"flanger",
                    tr("Flanger"),
                    flanger,
                    {{"period", div(4.0)}, {"depth", 0.8}, {"feedback", 0.6}}},
            {"phaser",
                    tr("Phaser"),
                    QStringLiteral("org.mixxx.effects.phaser"),
                    {{"period", div(1.0)}, {"feedback", 0.5}, {"amount", 1.0}}},
            {"crush",
                    tr("Bit crush"),
                    QStringLiteral("org.mixxx.effects.bitcrusher"),
                    {{"amount", 0.6}, {"bit_depth", 0.4}}},
            {"tapestop",
                    tr("Tape stop"),
                    QStringLiteral("org.mixxx.effects.tapestop"),
                    {{"time", div(1.0)}, {"quantize", 0.0}}},
            {"beat_repeat",
                    tr("Beat repeat"),
                    QStringLiteral("org.mixxx.effects.beatrepeat"),
                    {{"division", div(0.25)}}},
    };

    // BEAT FX on units 1 and 2. beatParam is the effect's beat-division
    // parameter; empty = no beats.
    m_beatFx = {
            {QStringLiteral("echo"),
                    tr("Echo"),
                    djEcho,
                    QStringLiteral("delay_time"),
                    {{"feedback", 0.6}, {"amount", 1.0}}},
            {QStringLiteral("spiral"),
                    tr("Spiral"),
                    QStringLiteral("org.mixxx.effects.spiral"),
                    QStringLiteral("delay_time"),
                    {{"feedback", 0.75}, {"amount", 1.0}}},
            {QStringLiteral("reverb"),
                    tr("Reverb"),
                    reverb,
                    QString(),
                    {{"send_amount", 1.0}, {"size", 0.8}, {"decay", 4.0}}},
            {QStringLiteral("trans"),
                    tr("Trans"),
                    tremolo,
                    QStringLiteral("rate"),
                    {{"depth", 1.0}, {"shape", 0.0}}},
            {QStringLiteral("flanger"),
                    tr("Flanger"),
                    flanger,
                    QStringLiteral("period"),
                    {{"depth", 0.8}, {"feedback", 0.6}}},
            {QStringLiteral("phaser"),
                    tr("Phaser"),
                    QStringLiteral("org.mixxx.effects.phaser"),
                    QStringLiteral("period"),
                    {{"feedback", 0.5}}},
            {QStringLiteral("roll"),
                    tr("Roll"),
                    QStringLiteral("org.mixxx.effects.beatrepeat"),
                    QStringLiteral("division"),
                    {}},
            {QStringLiteral("filter"),
                    tr("Filter"),
                    filter,
                    QString(),
                    {}},
            {QStringLiteral("crush"),
                    tr("Crush"),
                    QStringLiteral("org.mixxx.effects.bitcrusher"),
                    QString(),
                    {}},
            {QStringLiteral("tapestop"),
                    tr("Brake"),
                    QStringLiteral("org.mixxx.effects.tapestop"),
                    QStringLiteral("time"),
                    {}},
            {QStringLiteral("delay"),
                    tr("Tape Echo"),
                    QStringLiteral("org.mixxx.effects.vintagedelay"),
                    QStringLiteral("delay_time"),
                    {}},
            {QStringLiteral("pitch"),
                    tr("Pitch"),
                    QStringLiteral("org.mixxx.effects.pitchshift"),
                    QString(),
                    {}},
            {QStringLiteral("noise"),
                    tr("Noise"),
                    QStringLiteral("org.mixxx.effects.whitenoise"),
                    QString(),
                    {}},
            {QStringLiteral("glitch"),
                    tr("Stutter"),
                    QStringLiteral("org.mixxx.effects.glitch"),
                    QStringLiteral("step"),
                    {}},
            {QStringLiteral("space"),
                    tr("Space"),
                    QStringLiteral("org.mixxx.effects.pronoise"),
                    QString(),
                    {}},
            {QStringLiteral("pump"),
                    tr("Pump"),
                    QStringLiteral("org.mixxx.effects.procompressor"),
                    QStringLiteral("period"),
                    {}},
            {QStringLiteral("sweep"),
                    tr("Sweep"),
                    QStringLiteral("org.mixxx.effects.profilter"),
                    QString(),
                    {}},
            {QStringLiteral("shimmer"),
                    tr("Shimmer"),
                    QStringLiteral("org.mixxx.effects.proreverb"),
                    QString(),
                    {}},
    };
    m_pBeatFxEffect = std::make_unique<ControlObject>(
            ConfigKey(QStringLiteral("[BeatFx]"), QStringLiteral("effect")));
    m_pBeatFxBeats = std::make_unique<ControlObject>(
            ConfigKey(QStringLiteral("[BeatFx]"), QStringLiteral("beats")));
    m_pBeatFxBeats->set(1.0);
    m_pBeatFxLoad = std::make_unique<ControlPushButton>(
            ConfigKey(QStringLiteral("[BeatFx]"), QStringLiteral("load")));
    connect(m_pBeatFxLoad.get(), &ControlObject::valueChanged, this, [this](double value) {
        if (value > 0.0) {
            loadBeatFx();
        }
    });
    connect(m_pBeatFxEffect.get(), &ControlObject::valueChanged, this, [this](double) {
        loadBeatFx();
    });
    connect(m_pBeatFxBeats.get(), &ControlObject::valueChanged, this, [this](double) {
        loadBeatFx();
    });

    for (int deck = 0; deck < kNumDecks; ++deck) {
        const QString group = QStringLiteral("[PadFx_Channel%1]").arg(deck + 1);
        for (int pad = 1; pad <= kNumPads; ++pad) {
            auto pControl = std::make_unique<ControlPushButton>(
                    ConfigKey(group, QStringLiteral("pad%1").arg(pad)));
            connect(pControl.get(),
                    &ControlObject::valueChanged,
                    this,
                    [this, deck, pad](double value) { onPad(deck, pad, value); });
            m_padControls.push_back(std::move(pControl));
        }
    }
}

PadFxController::~PadFxController() = default;

void PadFxController::loadBeatFx() {
    const int index = ((static_cast<int>(m_pBeatFxEffect->get()) % kNumBeatFxSlots) +
                              kNumBeatFxSlots) %
            kNumBeatFxSlots;
    const BeatFx* pFx = beatFxEntry(beatFxSlot(index + 1));
    if (!pFx) {
        return;
    }
    const BeatFx& fx = *pFx;
    const double beats = std::clamp(m_pBeatFxBeats->get(), 1.0 / 16, 16.0);
    QList<QPair<QString, double>> parameters = fx.parameters;
    if (!fx.beatParam.isEmpty()) {
        parameters.append({fx.beatParam, static_cast<double>(djfx::divisionIndex(beats))});
    }
    const EffectPresetPointer pPreset = makePreset(fx.effectId, parameters, false);
    if (!pPreset) {
        return;
    }
    for (int unit = 0; unit < 2; ++unit) {
        const EffectChainPointer pChain = m_pEffectsManager->getStandardEffectChain(unit);
        const EffectSlotPointer pSlot = pChain ? pChain->getEffectSlot(0) : nullptr;
        if (!pSlot) {
            continue;
        }
        // Keep the slot's on/off state across the change
        const ConfigKey enabled(pSlot->getGroup(), QStringLiteral("enabled"));
        const double wasEnabled = ControlObject::get(enabled);
        pSlot->loadEffectFromPreset(pPreset);
        ControlObject::set(enabled, wasEnabled);
    }
}

const PadFxController::BeatFx* PadFxController::beatFxEntry(const QString& key) const {
    for (const BeatFx& fx : m_beatFx) {
        if (fx.key == key) {
            return &fx;
        }
    }
    return nullptr;
}

QVariantList PadFxController::beatFxCatalog() const {
    QVariantList list;
    for (const BeatFx& fx : m_beatFx) {
        list.append(QVariantMap{{QStringLiteral("key"), fx.key},
                {QStringLiteral("name"), fx.name}});
    }
    return list;
}

QString PadFxController::beatFxSlot(int slot) const {
    if (slot < 1 || slot > kNumBeatFxSlots) {
        return QString();
    }
    const QString fallback = m_beatFx[slot - 1].key;
    const QString key = m_pConfig->getValue(
            ConfigKey(kBeatFxConfigGroup, QStringLiteral("slot%1").arg(slot)), fallback);
    return beatFxEntry(key) ? key : fallback;
}

void PadFxController::setBeatFxSlot(int slot, const QString& key) {
    if (slot < 1 || slot > kNumBeatFxSlots || !beatFxEntry(key)) {
        return;
    }
    m_pConfig->setValue(
            ConfigKey(kBeatFxConfigGroup, QStringLiteral("slot%1").arg(slot)), key);
    if (static_cast<int>(m_pBeatFxEffect->get()) == slot - 1) {
        loadBeatFx();
    }
}

void PadFxController::resetBeatFx() {
    for (int slot = 1; slot <= kNumBeatFxSlots; ++slot) {
        setBeatFxSlot(slot, m_beatFx[slot - 1].key);
    }
}

QVariantList PadFxController::catalog() const {
    QVariantList list;
    for (const Entry& fx : m_catalog) {
        list.append(QVariantMap{{QStringLiteral("key"), fx.key},
                {QStringLiteral("name"), fx.name}});
    }
    return list;
}

QString PadFxController::padEffect(int pad) const {
    if (pad < 1 || pad > kNumPads) {
        return QString();
    }
    const QString key = m_pConfig->getValue(
            ConfigKey(kConfigGroup, QStringLiteral("pad%1").arg(pad)),
            QString::fromLatin1(kDefaultPads[pad - 1]));
    return entry(key) ? key : QString::fromLatin1(kDefaultPads[pad - 1]);
}

void PadFxController::setPadEffect(int pad, const QString& key) {
    if (pad < 1 || pad > kNumPads || !entry(key)) {
        return;
    }
    m_pConfig->setValue(ConfigKey(kConfigGroup, QStringLiteral("pad%1").arg(pad)), key);
}

void PadFxController::resetToDefaults() {
    for (int pad = 1; pad <= kNumPads; ++pad) {
        setPadEffect(pad, QString::fromLatin1(kDefaultPads[pad - 1]));
    }
}

const PadFxController::Entry* PadFxController::entry(const QString& key) const {
    for (const Entry& fx : m_catalog) {
        if (fx.key == key) {
            return &fx;
        }
    }
    return nullptr;
}

void PadFxController::onPad(int deck, int pad, double value) {
    if (value > 0.0) {
        const Entry* pFx = entry(padEffect(pad));
        if (!pFx) {
            return;
        }
        // A new pad replaces whatever the previous one held
        stop(deck);
        m_held[deck].pad = pad;
        start(deck, *pFx);
    } else if (m_held[deck].pad == pad) {
        stop(deck);
    }
}

EffectPresetPointer PadFxController::makePreset(const QString& effectId,
        const QList<QPair<QString, double>>& parameters,
        bool unlink) const {
    const auto pManifest = m_pEffectsManager->getBackendManager()->getManifest(
            effectId, EffectBackendType::BuiltIn);
    if (!pManifest) {
        return EffectPresetPointer();
    }
    const EffectPresetPointer pDefault =
            m_pEffectsManager->getEffectPresetManager()->getDefaultPreset(pManifest);
    if (!pDefault) {
        return EffectPresetPointer();
    }
    // Start from the effect's defaults and set the given values. Unlinked
    // from the meta knob, the values stay exactly as set.
    QDomDocument doc;
    QDomElement effect = pDefault->toXml(&doc);
    QDomNodeList params = effect.firstChildElement(EffectXml::kParametersRoot).childNodes();
    for (int i = 0; i < params.count(); ++i) {
        QDomElement param = params.at(i).toElement();
        const QString id = param.firstChildElement(EffectXml::kParameterId).text();
        QDomElement link = param.firstChildElement(EffectXml::kParameterLinkType);
        for (const auto& [paramId, paramValue] : parameters) {
            if (paramId != id) {
                continue;
            }
            param.firstChildElement(EffectXml::kParameterValue)
                    .firstChild()
                    .setNodeValue(QString::number(paramValue));
            // A set value must not be moved by the meta knob
            if (!link.isNull()) {
                link.firstChild().setNodeValue(QStringLiteral("NONE"));
            }
        }
        if (unlink && !link.isNull()) {
            link.firstChild().setNodeValue(QStringLiteral("NONE"));
        }
    }
    return EffectPresetPointer::create(effect);
}

void PadFxController::start(int deck, const Entry& fx) {
    if (fx.rollBeats > 0.0) {
        m_held[deck].rollBeats = fx.rollBeats;
        ControlObject::set(ConfigKey(channelGroup(deck), rollControl(fx.rollBeats)), 1.0);
        return;
    }
    const EffectChainPointer pChain =
            m_pEffectsManager->getStandardEffectChain(kUnitForDeck[deck] - 1);
    const EffectPresetPointer pPreset = makePreset(fx.effectId, fx.parameters, true);
    if (!pChain || !pPreset) {
        return;
    }

    const QList<EffectSlotPointer>& effectSlots = pChain->getEffectSlots();
    for (int i = 1; i < effectSlots.size(); ++i) {
        ControlObject::set(ConfigKey(effectSlots.at(i)->getGroup(),
                                   QStringLiteral("enabled")),
                0.0);
    }
    const EffectSlotPointer pSlot = pChain->getEffectSlot(0);
    if (!pSlot) {
        return;
    }
    pSlot->loadEffectFromPreset(pPreset);

    const QString unit = unitGroup(deck);
    ControlObject::set(ConfigKey(unit, QStringLiteral("mix_mode")),
            static_cast<double>(EffectChainMixMode::DryPlusWet));
    ControlObject::set(ConfigKey(unit, QStringLiteral("mix")), 1.0);
    for (int other = 0; other < kNumDecks; ++other) {
        ControlObject::set(ConfigKey(unit,
                                   QStringLiteral("group_%1_enable")
                                           .arg(channelGroup(other))),
                other == deck ? 1.0 : 0.0);
    }
    ControlObject::set(ConfigKey(pSlot->getGroup(), QStringLiteral("enabled")), 1.0);
    ControlObject::set(ConfigKey(unit, QStringLiteral("enabled")), 1.0);
    m_held[deck].effect = true;
}

void PadFxController::stop(int deck) {
    Held& held = m_held[deck];
    if (held.rollBeats > 0.0) {
        ControlObject::set(ConfigKey(channelGroup(deck), rollControl(held.rollBeats)), 0.0);
    }
    if (held.effect) {
        const EffectChainPointer pChain =
                m_pEffectsManager->getStandardEffectChain(kUnitForDeck[deck] - 1);
        const EffectSlotPointer pSlot = pChain ? pChain->getEffectSlot(0) : nullptr;
        if (pSlot) {
            ControlObject::set(ConfigKey(pSlot->getGroup(), QStringLiteral("enabled")), 0.0);
        }
    }
    held = Held();
}
