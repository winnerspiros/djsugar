.pragma library

// Texts shared by the stem separation screens (Android)

// The error codes of StemsWorker
function errorText(code) {
    switch (code) {
    case "":
        return "";
    case "canceled":
        return qsTr("Canceled");
    case "model":
        return qsTr("The model is missing or damaged: download it again in Settings → Stems");
    case "not-found":
        return qsTr("The file can't be found");
    case "decode":
        return qsTr("The audio of the file can't be read");
    case "too-long":
        return qsTr("Too long: up to 20 minutes per track");
    case "write":
        return qsTr("The stem file can't be saved");
    case "memory":
        return qsTr("Not enough memory: close other apps or use Spleeter");
    case "runtime":
        return qsTr("The AI engine can't be loaded on this device");
    default:
        return code;
    }
}

function duration(seconds) {
    if (seconds < 90) {
        return qsTr("%1 s").arg(Math.max(1, Math.round(seconds)));
    }
    return qsTr("%1 min").arg(Math.round(seconds / 60));
}

// Time to separate a 5 minute track at a measured speed (seconds of audio
// per second)
function trackTime(speed) {
    return duration(300 / speed);
}

function modelLabel(model) {
    return model === "htdemucs" ? "HTDemucs" : "Spleeter + Wiener";
}
