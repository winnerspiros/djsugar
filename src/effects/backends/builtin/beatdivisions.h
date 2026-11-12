#pragma once

// The beat-division table shared by the tempo-synced built-in effects and
// by PadFxController, which sets those parameters by step index.

#include <QList>
#include <QPair>
#include <QString>

namespace djfx {

// Beat divisions offered by every synced time or rate knob. Labels are
// in beats, like the Beat FX on club mixers (1/4 = a quarter of a beat).
inline const QList<QPair<QString, double>>& beatDivisions() {
    static const QList<QPair<QString, double>> kDivisions = {
            {QStringLiteral("1/8"), 0.125},
            {QStringLiteral("1/4"), 0.25},
            {QStringLiteral("1/3"), 1.0 / 3.0},
            {QStringLiteral("1/2"), 0.5},
            {QStringLiteral("2/3"), 2.0 / 3.0},
            {QStringLiteral("3/4"), 0.75},
            {QStringLiteral("1"), 1.0},
            {QStringLiteral("3/2"), 1.5},
            {QStringLiteral("2"), 2.0},
            {QStringLiteral("3"), 3.0},
            {QStringLiteral("4"), 4.0},
            {QStringLiteral("8"), 8.0},
            {QStringLiteral("16"), 16.0},
            {QStringLiteral("32"), 32.0},
    };
    return kDivisions;
}

// Index into beatDivisions() of a given number of beats.
inline int divisionIndex(double beats) {
    const auto& divisions = beatDivisions();
    for (int i = 0; i < divisions.size(); ++i) {
        if (divisions[i].second >= beats - 1e-6) {
            return i;
        }
    }
    return static_cast<int>(divisions.size()) - 1;
}

} // namespace djfx
