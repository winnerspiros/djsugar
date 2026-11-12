////////////////////////////////////////////////////////////////////////
// JSHint configuration                                               //
////////////////////////////////////////////////////////////////////////
/* global engine                                                      */
/* global script                                                      */
/* global midi                                                        */
/* global bpm                                                         */
/* global components                                                  */
////////////////////////////////////////////////////////////////////////
var PioneerDDJSX3 = function() {};

/*
	Author: 		DJMaxergy
	Version: 		1.19, 05/01/2018
	Description: 	Pioneer DDJ-SX Controller Mapping for Mixxx
    Source: 		http://github.com/DJMaxergy/mixxx/tree/pioneerDDJSX_mapping
    
    Copyright (c) 2018 DJMaxergy, licensed under GPL version 2 or later
    Copyright (c) 2014-2015 various contributors, base for this mapping, licensed under MIT license
    
    Contributors:
    - Michael Stahl (DG3NEC): original DDJ-SB2 mapping for Mixxx 2.0
    - Sophia Herzog: midiAutoDJ-scripts
    - Joan Ardiaca Jové (joan.ardiaca@gmail.com): Pioneer DDJ-SB mapping for Mixxx 2.0
    - wingcom (wwingcomm@gmail.com): start of Pioneer DDJ-SB mapping
      https://github.com/wingcom/Mixxx-Pioneer-DDJ-SB
    - Hilton Rudham: Pioneer DDJ-SR mapping
      https://github.com/hrudham/Mixxx-Pioneer-DDJ-SR
      
    GPL license notice for current version:
    This program is free software; you can redistribute it and/or modify it under the terms of the
    GNU General Public License as published by the Free Software Foundation; either version 2
    of the License, or (at your option) any later version.
    
    This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
    without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See
    the GNU General Public License for more details.
    
    You should have received a copy of the GNU General Public License along with this program; if
    not, write to the Free Software Foundation, Inc.,
    51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
    
    
    MIT License for earlier versions:
    Permission is hereby granted, free of charge, to any person obtaining a copy of this software
    and associated documentation files (the "Software"), to deal in the Software without
    restriction, including without limitation the rights to use, copy, modify, merge, publish,
    distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:
    
    The above copyright notice and this permission notice shall be included in all copies or
    substantial portions of the Software.
    
    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
    BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
    NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
    DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

///////////////////////////////////////////////////////////////
//                       USER OPTIONS                        //
///////////////////////////////////////////////////////////////

// Sets the jogwheels sensivity. 1 is default, 2 is twice as sensitive, 0.5 is half as sensitive.
PioneerDDJSX3.jogwheelSensivity = 1;

// Sets how much more sensitive the jogwheels get when holding shift.
// Set to 1 to disable jogwheel sensitivity increase when holding shift (default: 10).
PioneerDDJSX3.jogwheelShiftMultiplier = 10;

// If true, vu meters twinkle if AutoDJ is enabled (default: true).
//PioneerDDJSX3.twinkleVumeterAutodjOn = true;
PioneerDDJSX3.twinkleVumeterAutodjOn = false;
// If true, selected track will be added to AutoDJ queue-top on pressing shift + rotary selector,
// else track will be added to AutoDJ queue-bottom (default: false).
PioneerDDJSX3.autoDJAddTop = false;
// Sets the duration of sleeping between AutoDJ actions if AutoDJ is enabled [ms] (default: 1000).
PioneerDDJSX3.autoDJTickInterval = 1000;
// Sets the maximum adjustment of BPM allowed for beats to sync if AutoDJ is enabled [BPM] (default: 10).
PioneerDDJSX3.autoDJMaxBpmAdjustment = 10;
// If true, AutoDJ queue is being shuffled after skipping a track (default: false).
// When using a fixed set of tracks without manual intervention, some tracks may be unreachable, 
// due to having an unfortunate place in the queue ordering. This solves the issue.
PioneerDDJSX3.autoDJShuffleAfterSkip = false;

// If true, by releasing rotary selector, 
// track in preview player jumps forward to "jumpPreviewPosition"
// (default: jumpPreviewEnabled = true, jumpPreviewPosition = 0.3). 
PioneerDDJSX3.jumpPreviewEnabled = true;
PioneerDDJSX3.jumpPreviewPosition = 0.3;

// If true, pad press in SAMPLER-PAD-MODE repeatedly causes sampler to play 
// loaded track from cue-point, else it causes to play loaded track from the beginning (default: false).
PioneerDDJSX3.samplerCueGotoAndPlay = false;

// If true, PFL / Cue (headphone) is being activated by loading a track into certain deck (default: true).
PioneerDDJSX3.autoPFL = true;


///////////////////////////////////////////////////////////////
//               INIT, SHUTDOWN & GLOBAL HELPER              //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.shiftPressed = false;
PioneerDDJSX3.rotarySelectorChanged = false;
PioneerDDJSX3.panels = [false, false]; // view state of effect and sampler panel
PioneerDDJSX3.shiftPanelSelectPressed = false;

PioneerDDJSX3.syncRate = [0, 0, 0, 0];
PioneerDDJSX3.gridAdjustSelected = [false, false, false, false];
PioneerDDJSX3.gridSlideSelected = [false, false, false, false];
PioneerDDJSX3.needleSearchTouched = [false, false, false, false];
PioneerDDJSX3.chFaderStart = [null, null, null, null];
PioneerDDJSX3.toggledBrake = [false, false, false, false];
PioneerDDJSX3.scratchMode = [true, true, true, true];
PioneerDDJSX3.wheelLedsBlinkStatus = [0, 0, 0, 0];
PioneerDDJSX3.setUpSpeedSliderRange = [0.08, 0.08, 0.08, 0.08];
PioneerDDJSX3.serato=[0xF0,0x00,0x20,0x7F,0x50,0x01,0xF7];
PioneerDDJSX3.seratoPoll=[0xF0,0x00,0x20,0x7F,0x03,0x01,0xF7];

// PAD mode storage:
PioneerDDJSX3.padModes = {
    'hotCue': 0,
    'loopRoll': 1,
    'slicer': 2,
    'sampler': 3,
    'group1': 4,
    'beatloop': 5,
    'group3': 6,
    'group4': 7
};
PioneerDDJSX3.activePadMode = [
    PioneerDDJSX3.padModes.hotCue,
    PioneerDDJSX3.padModes.hotCue,
    PioneerDDJSX3.padModes.hotCue,
    PioneerDDJSX3.padModes.hotCue
];
PioneerDDJSX3.samplerVelocityMode = [false, false, false, false];

// FX storage:
PioneerDDJSX3.fxKnobMSBValue = [0, 0];
PioneerDDJSX3.shiftFxKnobMSBValue = [0, 0];

// used for advanced auto dj features:
PioneerDDJSX3.blinkAutodjState = false;
PioneerDDJSX3.autoDJTickTimer = 0;
PioneerDDJSX3.autoDJSyncBPM = false;
PioneerDDJSX3.autoDJSyncKey = false;

// used for PAD parameter selection:
PioneerDDJSX3.selectedSamplerBank = 0;
PioneerDDJSX3.selectedLoopParam = [0, 0, 0, 0];
PioneerDDJSX3.selectedLoopRollParam = [2, 2, 2, 2];
PioneerDDJSX3.selectedLoopIntervals = [
    [1 / 4, 1 / 2, 1, 2, 4, 8, 16, 32],
    [1 / 4, 1 / 2, 1, 2, 4, 8, 16, 32],
    [1 / 4, 1 / 2, 1, 2, 4, 8, 16, 32],
    [1 / 4, 1 / 2, 1, 2, 4, 8, 16, 32]
];
PioneerDDJSX3.selectedLooprollIntervals = [
    [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8],
    [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8],
    [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8],
    [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8]
];
PioneerDDJSX3.loopIntervals = [
    [1 / 4, 1 / 2, 1, 2, 4, 8, 16, 32],
    [1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8, 16],
    [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8],
    [1 / 32, 1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4]
];
PioneerDDJSX3.selectedSlicerQuantizeParam = [1, 1, 1, 1];
PioneerDDJSX3.selectedSlicerQuantization = [1 / 4, 1 / 4, 1 / 4, 1 / 4];
PioneerDDJSX3.slicerQuantizations = [1 / 8, 1 / 4, 1 / 2, 1];
PioneerDDJSX3.selectedSlicerDomainParam = [0, 0, 0, 0];
PioneerDDJSX3.selectedSlicerDomain = [8, 8, 8, 8];
PioneerDDJSX3.slicerDomains = [8, 16, 32, 64];

// slicer storage:
PioneerDDJSX3.slicerBeatsPassed = [0, 0, 0, 0];
PioneerDDJSX3.slicerPreviousBeatsPassed = [0, 0, 0, 0];
PioneerDDJSX3.slicerActive = [false, false, false, false];
PioneerDDJSX3.slicerAlreadyJumped = [false, false, false, false];
PioneerDDJSX3.slicerButton = [0, 0, 0, 0];
PioneerDDJSX3.slicerModes = {
    'contSlice': 0,
    'loopSlice': 1
};
PioneerDDJSX3.activeSlicerMode = [
    PioneerDDJSX3.slicerModes.contSlice,
    PioneerDDJSX3.slicerModes.contSlice,
    PioneerDDJSX3.slicerModes.contSlice,
    PioneerDDJSX3.slicerModes.contSlice
];


PioneerDDJSX3.init = function(id) {
    PioneerDDJSX3.scratchSettings = {
        'alpha': 1.0 / 8,
        'beta': 1.0 / 8 / 32,
        'jogResolution': 2048,
        'vinylSpeed': 33 + 1 / 3,
    };

    PioneerDDJSX3.channelGroups = {
        '[Channel1]': 0x00,
        '[Channel2]': 0x01,
        '[Channel3]': 0x02,
        '[Channel4]': 0x03
    };

    PioneerDDJSX3.samplerGroups = {
        '[Sampler1]': 0x00,
        '[Sampler2]': 0x01,
        '[Sampler3]': 0x02,
        '[Sampler4]': 0x03,
        '[Sampler5]': 0x04,
        '[Sampler6]': 0x05,
        '[Sampler7]': 0x06,
        '[Sampler8]': 0x07
    };

    PioneerDDJSX3.fxUnitGroups = {
        '[EffectRack1_EffectUnit1]': 0x00,
        '[EffectRack1_EffectUnit2]': 0x01
    };

    PioneerDDJSX3.fxEffectGroups = {
        '[EffectRack1_EffectUnit1_Effect1]': 0x00,
        '[EffectRack1_EffectUnit1_Effect2]': 0x01,
        '[EffectRack1_EffectUnit1_Effect3]': 0x02,
        '[EffectRack1_EffectUnit2_Effect1]': 0x00,
        '[EffectRack1_EffectUnit2_Effect2]': 0x01,
        '[EffectRack1_EffectUnit2_Effect3]': 0x02
    };

    PioneerDDJSX3.ledGroups = {
        'hotCue': 0x00,
        'loopRoll': 0x10,
        'slicer': 0x20,
        'sampler': 0x30,
        'group1': 0x40,
        'group2': 0x50,
        'group3': 0x60,
        'group4': 0x70
    };

    PioneerDDJSX3.nonPadLeds = {
        'headphoneCue': 0x54,
        'shiftHeadphoneCue': 0x68,
        'cue': 0x0C,
        'shiftCue': 0x48,
        'keyLock': 0x1A,
        'shiftKeyLock': 0x60,
        'play': 0x0B,
        'shiftPlay': 0x47,
        'vinyl': 0x0D,
        'sync': 0x58,
        'shiftSync': 0x5C,
        'autoLoop': 0x14,
        'shiftAutoLoop': 0x50,
        'loopHalve': 0x12,
        'shiftLoopHalve': 0x61,
        'loopDouble': 0x13,
        'shiftLoopDouble': 0x62,
        'loopIn': 0x10,
        'shiftLoopIn': 0x4C,
        'loopOut': 0x11,
        'shiftLoopOut': 0x4D,
        'censor': 0x15,
        'shiftCensor': 0x38,
        'slip': 0x40,
        'shiftSlip': 0x63,
        'gridAdjust': 0x79,
        'shiftGridAdjust': 0x64,
        'gridSlide': 0x0A,
        'shiftGridSlide': 0x65,
        'takeoverPlus': 0x34,
        'takeoverMinus': 0x37,
        'fx1on': 0x47,
        'shiftFx1on': 0x63,
        'fx2on': 0x48,
        'shiftFx2on': 0x64,
        'fx3on': 0x49,
        'shiftFx3on': 0x65,
        'fxTab': 0x4A,
        'shiftFxTab': 0x66,
        'fx1assignDeck1': 0x50,
        //'shiftFx1assignDeck1': 0x70,
        'fx1assignDeck2': 0x51,
        //'shiftFx1assignDeck2': 0x71,
        'fx1assignDeck3': 0x53,
        //'shiftFx1assignDeck3': 0x72,
        'fx1assignDeck4': 0x54,
        //'shiftFx1assignDeck4': 0x73,
        'fx2assignDeck1': 0x54,
        //'shiftFx2assignDeck1': 0x54,
        'fx2assignDeck2': 0x55,
        //'shiftFx2assignDeck2': 0x55,
        'fx2assignDeck3': 0x56,
        //'shiftFx2assignDeck3': 0x56,
        'fx2assignDeck4': 0x57,
        //'shiftFx2assignDeck4': 0x57,
        'masterCue': 0x63,
        'shiftMasterCue': 0x62,
        'loadDeck1': 0x46,
        'shiftLoadDeck1': 0x58,
        'loadDeck2': 0x47,
        'shiftLoadDeck2': 0x59,
        'loadDeck3': 0x48,
        'shiftLoadDeck3': 0x60,
        'loadDeck4': 0x49,
        'shiftLoadDeck4': 0x61,
        'hotCueMode': 0x1B,
        'shiftHotCueMode': 0x69,
        'rollMode': 0x1E,
        'shiftRollMode': 0x6B,
        'slicerMode': 0x20,
        'shiftSlicerMode': 0x6D,
        'samplerMode': 0x22,
        'shiftSamplerMode': 0x6F,
        'longPressSamplerMode': 0x41,
        'parameterLeftHotCueMode': 0x24,
        'shiftParameterLeftHotCueMode': 0x01,
        'parameterLeftRollMode': 0x25,
        'shiftParameterLeftRollMode': 0x02,
        'parameterLeftSlicerMode': 0x26,
        'shiftParameterLeftSlicerMode': 0x03,
        'parameterLeftSamplerMode': 0x27,
        'shiftParameterLeftSamplerMode': 0x04,
        'parameterLeftGroup1Mode': 0x28,
        'shiftParameterLeftGroup1Mode': 0x05,
        'parameterLeftGroup2Mode': 0x29,
        'shiftParameterLeftGroup2Mode': 0x06,
        'parameterLeftGroup3Mode': 0x2A,
        'shiftParameterLeftGroup3Mode': 0x07,
        'parameterLeftGroup4Mode': 0x2B,
        'shiftParameterLeftGroup4Mode': 0x08,
        'parameterRightHotCueMode': 0x2C,
        'shiftParameterRightHotCueMode': 0x09,
        'parameterRightRollMode': 0x2D,
        'shiftParameterRightRollMode': 0x7A,
        'parameterRightSlicerMode': 0x2E,
        'shiftParameterRightSlicerMode': 0x7B,
        'parameterRightSamplerMode': 0x2F,
        'shiftParameterRightSamplerMode': 0x7C,
        'parameterRightGroup1Mode': 0x30,
        'shiftParameterRightGroup1Mode': 0x7D,
        'parameterRightGroup2Mode': 0x31,
        'shiftParameterRightGroup2Mode': 0x7E,
        'parameterRightGroup3Mode': 0x32,
        'shiftParameterRightGroup3Mode': 0x7F,
        'parameterRightGroup4Mode': 0x33,
        'shiftParameterRightGroup4Mode': 0x00
    };

    PioneerDDJSX3.illuminationControl = {
        'loadedDeck1': 0x00,
        'loadedDeck2': 0x01,
        'loadedDeck3': 0x02,
        'loadedDeck4': 0x03,
        'unknownDeck1': 0x04,
        'unknownDeck2': 0x05,
        'unknownDeck3': 0x06,
        'unknownDeck4': 0x07,
        'playPauseDeck1': 0x0C,
        'playPauseDeck2': 0x0D,
        'playPauseDeck3': 0x0E,
        'playPauseDeck4': 0x0F,
        'cueDeck1': 0x10,
        'cueDeck2': 0x11,
        'cueDeck3': 0x12,
        'cueDeck4': 0x13,
        'djAppConnect': 0x09
    };

    PioneerDDJSX3.wheelLedCircle = {
        'minVal': 0x00,
        'maxVal': 0x48
    };

    PioneerDDJSX3.valueVuMeter = {
        '[Channel1]_current': 0,
        '[Channel2]_current': 0,
        '[Channel3]_current': 0,
        '[Channel4]_current': 0,
        '[Channel1]_enabled': 1,
        '[Channel2]_enabled': 1,
        '[Channel3]_enabled': 1,
        '[Channel4]_enabled': 1
    };

    // set 32 Samplers as default:
    engine.setValue("[Master]", "num_samplers", 32);

    // activate vu meter timer for Auto DJ:
    if (PioneerDDJSX3.twinkleVumeterAutodjOn) {
        PioneerDDJSX3.vuMeterTimer = engine.beginTimer(200, "PioneerDDJSX3.vuMeterTwinkle()");
    }

    // initiate control status request:
    midi.sendShortMsg(0x9B, 0x08, 0x7F);

    // bind controls and init deck parameters:
    PioneerDDJSX3.bindNonDeckControlConnections(true);
    for (var index in PioneerDDJSX3.channelGroups) {
        if (PioneerDDJSX3.channelGroups.hasOwnProperty(index)) {
            PioneerDDJSX3.initDeck(index);
        }
    }

    // init effects section:
    PioneerDDJSX3.effectUnit = [];
    PioneerDDJSX3.effectUnit[1] = new components.EffectUnit([1]);
    PioneerDDJSX3.effectUnit[2] = new components.EffectUnit([2]);
    PioneerDDJSX3.effectUnit[1].enableButtons[1].midi = [0x94, PioneerDDJSX3.nonPadLeds.fx1on];
    PioneerDDJSX3.effectUnit[1].enableButtons[2].midi = [0x94, PioneerDDJSX3.nonPadLeds.fx2on];
    PioneerDDJSX3.effectUnit[1].enableButtons[3].midi = [0x94, PioneerDDJSX3.nonPadLeds.fx3on];
    PioneerDDJSX3.effectUnit[1].effectFocusButton.midi = [0x94, PioneerDDJSX3.nonPadLeds.fxTab];
    PioneerDDJSX3.effectUnit[1].dryWetKnob.input = function(channel, control, value, status, group) {
        this.inSetParameter(this.inGetParameter() + PioneerDDJSX3.getRotaryDelta(value) / 30);
    };
    PioneerDDJSX3.effectUnit[1].init();
    PioneerDDJSX3.effectUnit[2].enableButtons[1].midi = [0x95, PioneerDDJSX3.nonPadLeds.fx1on];
    PioneerDDJSX3.effectUnit[2].enableButtons[2].midi = [0x95, PioneerDDJSX3.nonPadLeds.fx2on];
    PioneerDDJSX3.effectUnit[2].enableButtons[3].midi = [0x95, PioneerDDJSX3.nonPadLeds.fx3on];
    PioneerDDJSX3.effectUnit[2].effectFocusButton.midi = [0x95, PioneerDDJSX3.nonPadLeds.fxTab];
    PioneerDDJSX3.effectUnit[2].dryWetKnob.input = function(channel, control, value, status, group) {
        this.inSetParameter(this.inGetParameter() + PioneerDDJSX3.getRotaryDelta(value) / 30);
    };
    PioneerDDJSX3.effectUnit[2].init();

    PioneerDDJSX3.seratoTimer = engine.beginTimer(250, "PioneerDDJSX3.doTimer", 0);
    midi.sendSysexMsg(PioneerDDJSX3.seratoPoll, PioneerDDJSX3.seratoPoll.length);
    //sleep(4000);
};

PioneerDDJSX3.doTimer = function(){
  midi.sendSysexMsg(PioneerDDJSX3.serato,PioneerDDJSX3.serato.length);
};

PioneerDDJSX3.shutdown = function() {
    PioneerDDJSX3.resetDeck("[Channel1]");
    PioneerDDJSX3.resetDeck("[Channel2]");
    PioneerDDJSX3.resetDeck("[Channel3]");
    PioneerDDJSX3.resetDeck("[Channel4]");

    PioneerDDJSX3.resetNonDeckLeds();
};


///////////////////////////////////////////////////////////////
//                      VU - METER                           //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.vuMeterTwinkle = function() {
    if (engine.getValue("[AutoDJ]", "enabled")) {
        PioneerDDJSX3.blinkAutodjState = !PioneerDDJSX3.blinkAutodjState;
    }
    PioneerDDJSX3.valueVuMeter["[Channel1]_enabled"] = PioneerDDJSX3.blinkAutodjState ? 1 : 0;
    PioneerDDJSX3.valueVuMeter["[Channel3]_enabled"] = PioneerDDJSX3.blinkAutodjState ? 1 : 0;
    PioneerDDJSX3.valueVuMeter["[Channel2]_enabled"] = PioneerDDJSX3.blinkAutodjState ? 1 : 0;
    PioneerDDJSX3.valueVuMeter["[Channel4]_enabled"] = PioneerDDJSX3.blinkAutodjState ? 1 : 0;
};


///////////////////////////////////////////////////////////////
//                        AUTO DJ                            //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.autodjToggle = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl("[AutoDJ]", "enabled");
    }
};

PioneerDDJSX3.autoDJToggleSyncBPM = function(channel, control, value, status, group) {
    if (value) {
        PioneerDDJSX3.autoDJSyncBPM = !PioneerDDJSX3.autoDJSyncBPM;
        PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck1, PioneerDDJSX3.autoDJSyncBPM);
    }
};

PioneerDDJSX3.autoDJToggleSyncKey = function(channel, control, value, status, group) {
    if (value) {
        PioneerDDJSX3.autoDJSyncKey = !PioneerDDJSX3.autoDJSyncKey;
        PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck2, PioneerDDJSX3.autoDJSyncKey);
    }
};

PioneerDDJSX3.autoDJTimer = function(value, group, control) {
    if (value) {
        PioneerDDJSX3.autoDJTickTimer = engine.beginTimer(PioneerDDJSX3.autoDJTickInterval, "PioneerDDJSX3.autoDJControl()");
    } else if (PioneerDDJSX3.autoDJTickTimer) {
        engine.stopTimer(PioneerDDJSX3.autoDJTickTimer);
        PioneerDDJSX3.autoDJTickTimer = 0;
    }
    engine.setValue("[Channel1]", "quantize", value);
    engine.setValue("[Channel2]", "quantize", value);
};

PioneerDDJSX3.autoDJControl = function() {
    var prev = 1,
        next = 2,
        prevPos = 0,
        nextPos = 0,
        nextPlaying = 0,
        prevBpm = 0,
        nextBpm = 0,
        diffBpm = 0,
        diffBpmDouble = 0,
        keyOkay = 0,
        prevKey = 0,
        nextKey = 0,
        diffKey = 0;

    if (!PioneerDDJSX3.autoDJSyncBPM && !PioneerDDJSX3.autoDJSyncKey) {
        return;
    }

    prevPos = engine.getValue("[Channel" + prev + "]", "playposition");
    nextPos = engine.getValue("[Channel" + next + "]", "playposition");
    if (prevPos < nextPos) {
        var tmp = nextPos;
        nextPos = prevPos;
        prevPos = tmp;
        next = 1;
        prev = 2;
    }
    nextPlaying = engine.getValue("[Channel" + next + "]", "play_indicator");
    prevBpm = engine.getValue("[Channel" + prev + "]", "visual_bpm");
    nextBpm = engine.getValue("[Channel" + next + "]", "visual_bpm");
    diffBpm = Math.abs(nextBpm - prevBpm);
    // diffBpm, with bpm of ONE track doubled
    // Note: Where appropriate, Mixxx will automatically match two beats of one.
    if (nextBpm < prevBpm) {
        diffBpmDouble = Math.abs(2 * nextBpm - prevBpm);
    } else {
        diffBpmDouble = Math.abs(2 * prevBpm - nextBpm);
    }

    // Next track is playing --> Fade in progress
    // Note: play_indicator is falsely true, when analysis is needed and similar
    if (nextPlaying && (nextPos > 0.0)) {
        // Bpm synced up --> disable sync before new track loaded
        // Note: Sometimes, Mixxx does not sync close enough for === operator
        if (diffBpm < 0.01 || diffBpmDouble < 0.01) {
            engine.setValue("[Channel" + prev + "]", "sync_mode", 0.0);
            engine.setValue("[Channel" + next + "]", "sync_mode", 0.0);
        } else { // Synchronize
            engine.setValue("[Channel" + prev + "]", "sync_mode", 1.0); // First,  set prev to follower
            engine.setValue("[Channel" + next + "]", "sync_mode", 2.0); // Second, set next to master
        }

        // Only adjust key when approaching the middle of fading
        if (PioneerDDJSX3.autoDJSyncKey) {
            var diffFader = Math.abs(engine.getValue("[Master]", "crossfader") - 0.5);
            if (diffFader < 0.25) {
                nextKey = engine.getValue("[Channel" + next + "]", "key");
                engine.setValue("[Channel" + prev + "]", "key", nextKey);
            }
        }
    } else if (!nextPlaying) { // Next track is stopped --> Disable sync and refine track selection
        // First, disable sync; should be off by now, anyway
        engine.setValue("[Channel" + prev + "]", "sync_mode", 0.0); // Disable sync, else loading new track...
        engine.setValue("[Channel" + next + "]", "sync_mode", 0.0); // ...or skipping tracks would break things.

        // Second, refine track selection
        var skip = 0;
        if (diffBpm > PioneerDDJSX3.autoDJMaxBpmAdjustment && diffBpmDouble > PioneerDDJSX3.autoDJMaxBpmAdjustment) {
            skip = 1;
        }
        // Mixing in key:
        //     1  the difference is exactly 12 (harmonic switch of tonality), or
        //     2  both are of same tonality, and
        //     2a difference is 0, 1 or 2 (difference of up to two semitones: equal key or energy mix)
        //     2b difference corresponds to neighbours in the circle of fifth (harmonic neighbours)
        //   If neither is the case, we skip.
        if (PioneerDDJSX3.autoDJSyncKey) {
            keyOkay = 0;
            prevKey = engine.getValue("[Channel" + prev + "]", "visual_key");
            nextKey = engine.getValue("[Channel" + next + "]", "visual_key");
            diffKey = Math.abs(prevKey - nextKey);
            if (diffKey === 12.0) {
                keyOkay = 1; // Switch of tonality
            }
            // Both of same tonality:
            if ((prevKey < 13 && nextKey < 13) || (prevKey > 12 && nextKey > 12)) {
                if (diffKey < 3.0) {
                    keyOkay = 1; // Equal or Energy
                }
                if (diffKey === 5.0 || diffKey === 7.0) {
                    keyOkay = 1; // Neighbours in Circle of Fifth
                }
            }
            if (!keyOkay) {
                skip = 1;
            }
        }

        if (skip) {
            engine.setValue("[AutoDJ]", "skip_next", 1.0);
            engine.setValue("[AutoDJ]", "skip_next", 0.0); // Have to reset manually
            if (PioneerDDJSX3.autoDJShuffleAfterSkip) {
                engine.setValue("[AutoDJ]", "shuffle_playlist", 1.0);
                engine.setValue("[AutoDJ]", "shuffle_playlist", 0.0); // Have to reset manually
            }
        }
    }
};


///////////////////////////////////////////////////////////////
//                      CONTROL BINDING                      //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.bindDeckControlConnections = function(channelGroup, bind) {
    var i,
        index,
        deck = PioneerDDJSX3.channelGroups[channelGroup],
        controlsToFunctions = {
            'play_indicator': 'PioneerDDJSX3.playLed',
            'cue_indicator': 'PioneerDDJSX3.cueLed',
            'playposition': 'PioneerDDJSX3.wheelLeds',
            'pfl': 'PioneerDDJSX3.headphoneCueLed',
            'bpm_tap': 'PioneerDDJSX3.shiftHeadphoneCueLed',
            'VuMeter': 'PioneerDDJSX3.VuMeterLeds',
            'keylock': 'PioneerDDJSX3.keyLockLed',
            'slip_enabled': 'PioneerDDJSX3.slipLed',
            'quantize': 'PioneerDDJSX3.quantizeLed',
            'loop_in': 'PioneerDDJSX3.loopInLed',
            'loop_out': 'PioneerDDJSX3.loopOutLed',
            'loop_enabled': 'PioneerDDJSX3.autoLoopLed',
            'loop_double': 'PioneerDDJSX3.loopDoubleLed',
            'loop_halve': 'PioneerDDJSX3.loopHalveLed',
            'reloop_andstop': 'PioneerDDJSX3.shiftLoopInLed',
            'beatjump_1_forward': 'PioneerDDJSX3.loopShiftFWLed',
            'beatjump_1_backward': 'PioneerDDJSX3.loopShiftBKWLed',
            'beatjump_forward': 'PioneerDDJSX3.hotCueParameterRightLed',
            'beatjump_backward': 'PioneerDDJSX3.hotCueParameterLeftLed',
            'reverse': 'PioneerDDJSX3.reverseLed',
            'duration': 'PioneerDDJSX3.loadLed',
            'sync_enabled': 'PioneerDDJSX3.syncLed',
            'beat_active': 'PioneerDDJSX3.slicerBeatActive'
        };

    for (i = 1; i <= 8; i++) {
        controlsToFunctions["hotcue_" + i + "_enabled"] = "PioneerDDJSX3.hotCueLeds";
    }

    for (index in PioneerDDJSX3.selectedLoopIntervals[deck]) {
        if (PioneerDDJSX3.selectedLoopIntervals[deck].hasOwnProperty(index)) {
            controlsToFunctions["beatloop_" + PioneerDDJSX3.selectedLoopIntervals[deck][index] + "_enabled"] = "PioneerDDJSX3.beatloopLeds";
        }
    }

    for (index in PioneerDDJSX3.selectedLooprollIntervals[deck]) {
        if (PioneerDDJSX3.selectedLooprollIntervals[deck].hasOwnProperty(index)) {
            controlsToFunctions["beatlooproll_" + PioneerDDJSX3.selectedLooprollIntervals[deck][index] + "_activate"] = "PioneerDDJSX3.beatlooprollLeds";
        }
    }

    script.bindConnections(channelGroup, controlsToFunctions, !bind);

    for (index in PioneerDDJSX3.fxUnitGroups) {
        if (PioneerDDJSX3.fxUnitGroups.hasOwnProperty(index)) {
            if (PioneerDDJSX3.fxUnitGroups[index] < 2) {
                engine.connectControl(index, "group_" + channelGroup + "_enable", "PioneerDDJSX3.fxAssignLeds", !bind);
                if (bind) {
                    engine.trigger(index, "group_" + channelGroup + "_enable");
                }
            }
        }
    }
};

PioneerDDJSX3.bindNonDeckControlConnections = function(bind) {
    var index;

    for (index in PioneerDDJSX3.samplerGroups) {
        if (PioneerDDJSX3.samplerGroups.hasOwnProperty(index)) {
            engine.connectControl(index, "duration", "PioneerDDJSX3.samplerLeds", !bind);
            engine.connectControl(index, "play", "PioneerDDJSX3.samplerLedsPlay", !bind);
            if (bind) {
                engine.trigger(index, "duration");
            }
        }
    }

    engine.connectControl("[Master]", "headSplit", "PioneerDDJSX3.shiftMasterCueLed", !bind);
    if (bind) {
        engine.trigger("[Master]", "headSplit");
    }

    engine.connectControl("[AutoDJ]", "enabled", "PioneerDDJSX3.autoDJTimer", !bind);
};


///////////////////////////////////////////////////////////////
//                     DECK INIT / RESET                     //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.initDeck = function(group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    // save set up speed slider range from the Mixxx settings:
    PioneerDDJSX3.setUpSpeedSliderRange[deck] = engine.getValue(group, "rateRange");

    PioneerDDJSX3.bindDeckControlConnections(group, true);

    PioneerDDJSX3.updateParameterStatusLeds(
        group,
        PioneerDDJSX3.selectedLoopRollParam[deck],
        PioneerDDJSX3.selectedLoopParam[deck],
        PioneerDDJSX3.selectedSamplerBank,
        PioneerDDJSX3.selectedSlicerQuantizeParam[deck],
        PioneerDDJSX3.selectedSlicerDomainParam[deck]
    );
    PioneerDDJSX3.triggerVinylLed(deck);

    PioneerDDJSX3.illuminateFunctionControl(
        PioneerDDJSX3.illuminationControl["loadedDeck" + (deck + 1)],
        false
    );
    PioneerDDJSX3.illuminateFunctionControl(
        PioneerDDJSX3.illuminationControl["unknownDeck" + (deck + 1)],
        false
    );
    PioneerDDJSX3.wheelLedControl(group, PioneerDDJSX3.wheelLedCircle.minVal);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.hotCueMode, true); // set HOT CUE Pad-Mode
};

PioneerDDJSX3.resetDeck = function(group) {
    PioneerDDJSX3.bindDeckControlConnections(group, false);

    PioneerDDJSX3.VuMeterLeds(0x00, group, 0x00); // reset VU meter Leds
    PioneerDDJSX3.wheelLedControl(group, PioneerDDJSX3.wheelLedCircle.minVal); // reset jogwheel Leds
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.hotCueMode, true); // reset HOT CUE Pad-Mode
    // pad Leds:
    for (var i = 0; i < 8; i++) {
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.hotCue, i, false, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.loopRoll, i, false, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.slicer, i, false, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.sampler, i, false, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.group2, i, false, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.hotCue, i, true, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.loopRoll, i, true, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.slicer, i, true, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.sampler, i, true, false);
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.group2, i, true, false);
    }
    // non pad Leds:
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.headphoneCue, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftHeadphoneCue, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.cue, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftCue, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.keyLock, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftKeyLock, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.play, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftPlay, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.vinyl, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.sync, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftSync, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.autoLoop, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftAutoLoop, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopHalve, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopHalve, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopIn, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopIn, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopOut, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopOut, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.censor, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftCensor, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.slip, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftSlip, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.gridAdjust, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftGridAdjust, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.gridSlide, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftGridSlide, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftRollMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftSlicerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterLeftSlicerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftSamplerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftGroup2Mode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightRollMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightSlicerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterRightSlicerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightSamplerMode, false);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightGroup2Mode, false);
};


///////////////////////////////////////////////////////////////
//            HIGH RESOLUTION MIDI INPUT HANDLERS            //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.highResMSB = {
    '[Channel1]': {},
    '[Channel2]': {},
    '[Channel3]': {},
    '[Channel4]': {},
    '[Master]': {},
    '[Samplers]': {}
};

PioneerDDJSX3.tempoSliderMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].tempoSlider = value;
};

PioneerDDJSX3.tempoSliderLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].tempoSlider << 7) + value,
        sliderRate = 1 - (fullValue / 0x3FFF),
        deck = PioneerDDJSX3.channelGroups[group];

    engine.setParameter(group, "rate", sliderRate);

    if (PioneerDDJSX3.syncRate[deck] !== 0) {
        if (PioneerDDJSX3.syncRate[deck] !== engine.getValue(group, "rate")) {
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, 0);
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, 0);
            PioneerDDJSX3.syncRate[deck] = 0;
        }
    }
};

PioneerDDJSX3.gainKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].gainKnob = value;
};

PioneerDDJSX3.gainKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].gainKnob << 7) + value;
    engine.setParameter(group, "pregain", fullValue / 0x3FFF);
};

PioneerDDJSX3.filterHighKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].filterHigh = value;
};

PioneerDDJSX3.filterHighKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].filterHigh << 7) + value;
    engine.setParameter("[EqualizerRack1_" + group + "_Effect1]", "parameter3", fullValue / 0x3FFF);
};

PioneerDDJSX3.filterMidKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].filterMid = value;
};

PioneerDDJSX3.filterMidKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].filterMid << 7) + value;
    engine.setParameter("[EqualizerRack1_" + group + "_Effect1]", "parameter2", fullValue / 0x3FFF);
};

PioneerDDJSX3.filterLowKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].filterLow = value;
};

PioneerDDJSX3.filterLowKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].filterLow << 7) + value;
    engine.setParameter("[EqualizerRack1_" + group + "_Effect1]", "parameter1", fullValue / 0x3FFF);
};

PioneerDDJSX3.deckFaderMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].deckFader = value;
};

PioneerDDJSX3.deckFaderLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].deckFader << 7) + value;

    if (PioneerDDJSX3.shiftPressed &&
        engine.getValue(group, "volume") === 0 &&
        fullValue !== 0 &&
        engine.getValue(group, "play") === 0
    ) {
        PioneerDDJSX3.chFaderStart[channel] = engine.getValue(group, "playposition");
        engine.setValue(group, "play", 1);
    } else if (
        PioneerDDJSX3.shiftPressed &&
        engine.getValue(group, "volume") !== 0 &&
        fullValue === 0 &&
        engine.getValue(group, "play") === 1 &&
        PioneerDDJSX3.chFaderStart[channel] !== null
    ) {
        engine.setValue(group, "play", 0);
        engine.setValue(group, "playposition", PioneerDDJSX3.chFaderStart[channel]);
        PioneerDDJSX3.chFaderStart[channel] = null;
    }
    engine.setParameter(group, "volume", fullValue / 0x3FFF);
};

PioneerDDJSX3.filterKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].filterKnob = value;
};

PioneerDDJSX3.filterKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].filterKnob << 7) + value;
    engine.setParameter("[QuickEffectRack1_" + group + "]", "super1", fullValue / 0x3FFF);
};

PioneerDDJSX3.crossfaderCurveKnobMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].crossfaderCurveKnob = value;
};

PioneerDDJSX3.crossfaderCurveKnobLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].crossfaderCurveKnob << 7) + value;
    script.crossfaderCurve(fullValue, 0x00, 0x3FFF);
};

PioneerDDJSX3.samplerVolumeFaderMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].samplerVolumeFader = value;
};

PioneerDDJSX3.samplerVolumeFaderLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].samplerVolumeFader << 7) + value;
    for (var i = 1; i <= 32; i++) {
        engine.setParameter("[Sampler" + i + "]", "volume", fullValue / 0x3FFF);
    }
};

PioneerDDJSX3.crossFaderMSB = function(channel, control, value, status, group) {
    PioneerDDJSX3.highResMSB[group].crossFader = value;
};

PioneerDDJSX3.crossFaderLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJSX3.highResMSB[group].crossFader << 7) + value;
    engine.setParameter(group, "crossfader", fullValue / 0x3FFF);
};


///////////////////////////////////////////////////////////////
//           SINGLE MESSAGE MIDI INPUT HANDLERS              //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.shiftButton = function(channel, control, value, status, group) {
    var index = 0;
    PioneerDDJSX3.shiftPressed = (value === 0x7F);
    for (index in PioneerDDJSX3.chFaderStart) {
        if (typeof index === "number") {
            PioneerDDJSX3.chFaderStart[index] = null;
        }
    }
    if (value) {
        PioneerDDJSX3.effectUnit[1].shift();
        PioneerDDJSX3.effectUnit[2].shift();
    }
    if (!value) {
        PioneerDDJSX3.effectUnit[1].unshift();
        PioneerDDJSX3.effectUnit[2].unshift();
    }
};

PioneerDDJSX3.playButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group],
        playing = engine.getValue(group, "play");

    if (value) {
        if (playing) {
            script.brake(channel, control, value, status, group);
            PioneerDDJSX3.toggledBrake[deck] = true;
        } else {
            script.toggleControl(group, "play");
        }
    } else {
        if (PioneerDDJSX3.toggledBrake[deck]) {
            script.brake(channel, control, value, status, group);
            script.toggleControl(group, "play");
            PioneerDDJSX3.toggledBrake[deck] = false;
        }
    }
};

PioneerDDJSX3.playStutterButton = function(channel, control, value, status, group) {
    engine.setValue(group, "play_stutter", value ? 1 : 0);
};

PioneerDDJSX3.cueButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "cue_default");
};

PioneerDDJSX3.jumpToBeginningButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "start_stop");
};

PioneerDDJSX3.headphoneCueButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "pfl");
    }
};

PioneerDDJSX3.headphoneShiftCueButton = function(channel, control, value, status, group) {
    if (value) {
        bpm.tapButton(PioneerDDJSX3.channelGroups[group] + 1);
    }
};

PioneerDDJSX3.headphoneSplitCueButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "headSplit");
    }
};

PioneerDDJSX3.toggleHotCueMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    //HOTCUE
    if (value) {
        PioneerDDJSX3.activePadMode[deck] = PioneerDDJSX3.padModes.hotCue;
        PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.contSlice;
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.hotCueMode, value);
    }
};

PioneerDDJSX3.toggleBeatloopRollMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    //ROLL
    if (value) {
        PioneerDDJSX3.activePadMode[deck] = PioneerDDJSX3.padModes.loopRoll;
        PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.contSlice;
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.rollMode, value);
    }
};

PioneerDDJSX3.toggleSlicerMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    //SLICER
    if (value) {
        if (PioneerDDJSX3.activePadMode[deck] === PioneerDDJSX3.padModes.slicer &&
            PioneerDDJSX3.activeSlicerMode[deck] === PioneerDDJSX3.slicerModes.contSlice) {
            PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.loopSlice;
            engine.setValue(group, "slip_enabled", true);
        } else {
            PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.contSlice;
            engine.setValue(group, "slip_enabled", false);
        }
        PioneerDDJSX3.activePadMode[deck] = PioneerDDJSX3.padModes.slicer;
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.slicerMode, value);
    }
};

PioneerDDJSX3.toggleSamplerMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    //SAMPLER
    if (value) {
        PioneerDDJSX3.activePadMode[deck] = PioneerDDJSX3.padModes.sampler;
        PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.contSlice;
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.samplerMode, value);
    }
};

PioneerDDJSX3.toggleSamplerVelocityMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group],
        index = 0;
    PioneerDDJSX3.samplerVelocityMode[deck] = value ? true : false;
    if (value) {
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.longPressSamplerMode, value);
        for (index = 1; index <= 32; index++) {
            engine.setParameter("[Sampler" + index + "]", "volume", 0);
        }
    } else {
        for (index = 1; index <= 32; index++) {
            engine.setParameter("[Sampler" + index + "]", "volume", 1);
        }
    }
};

PioneerDDJSX3.toggleBeatloopMode = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    //GROUP2
    if (value) {
        PioneerDDJSX3.activePadMode[deck] = PioneerDDJSX3.padModes.beatloop;
        PioneerDDJSX3.activeSlicerMode[deck] = PioneerDDJSX3.slicerModes.contSlice;
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftRollMode, value);
    }
};

PioneerDDJSX3.hotCueButtons = function(channel, control, value, status, group) {
    var index = control + 1;
    script.toggleControl(group, "hotcue_" + index + "_activate");
};

PioneerDDJSX3.clearHotCueButtons = function(channel, control, value, status, group) {
    var index = control - 0x08 + 1;
    script.toggleControl(group, "hotcue_" + index + "_clear");
};

PioneerDDJSX3.beatloopButtons = function(channel, control, value, status, group) {
    var index = control - 0x50,
        deck = PioneerDDJSX3.channelGroups[group];
    script.toggleControl(
        group,
        "beatloop_" + PioneerDDJSX3.selectedLoopIntervals[deck][index] + "_toggle"
    );
};

PioneerDDJSX3.slicerButtons = function(channel, control, value, status, group) {
    var index = control - 0x20,
        deck = PioneerDDJSX3.channelGroups[group],
        domain = PioneerDDJSX3.selectedSlicerDomain[deck],
        beatsToJump = 0;

    if (PioneerDDJSX3.activeSlicerMode[deck] === PioneerDDJSX3.slicerModes.loopSlice) {
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.slicer, index, false, !value);
    } else {
        PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.slicer, index, false, value);
    }
    PioneerDDJSX3.slicerActive[deck] = value ? true : false;
    PioneerDDJSX3.slicerButton[deck] = index;

    if (value) {
        beatsToJump = (PioneerDDJSX3.slicerButton[deck] * (domain / 8)) - ((PioneerDDJSX3.slicerBeatsPassed[deck] % domain) + 1);
        if (PioneerDDJSX3.slicerButton[deck] === 0 && beatsToJump === -domain) {
            beatsToJump = 0;
        }
        if (PioneerDDJSX3.slicerBeatsPassed[deck] >= Math.abs(beatsToJump) &&
            PioneerDDJSX3.slicerPreviousBeatsPassed[deck] !== PioneerDDJSX3.slicerBeatsPassed[deck]) {
            PioneerDDJSX3.slicerPreviousBeatsPassed[deck] = PioneerDDJSX3.slicerBeatsPassed[deck];
            if (Math.abs(beatsToJump) > 0) {
                engine.setValue(group, "beatjump", beatsToJump);
            }
        }
    }

    if (PioneerDDJSX3.activeSlicerMode[deck] === PioneerDDJSX3.slicerModes.contSlice) {
        engine.setValue(group, "slip_enabled", value);
        engine.setValue(group, "beatloop_size", PioneerDDJSX3.selectedSlicerQuantization[deck]);
        engine.setValue(group, "beatloop_activate", value);
    }
};

PioneerDDJSX3.beatloopRollButtons = function(channel, control, value, status, group) {
    var index = control - 0x10,
        deck = PioneerDDJSX3.channelGroups[group];
    script.toggleControl(
        group,
        "beatlooproll_" + PioneerDDJSX3.selectedLooprollIntervals[deck][index] + "_activate"
    );
};

PioneerDDJSX3.samplerButtons = function(channel, control, value, status, group) {
    var index = control - 0x30 + 1,
        deckOffset = PioneerDDJSX3.selectedSamplerBank * 8,
        sampleDeck = "[Sampler" + (index + deckOffset) + "]",
        playMode = PioneerDDJSX3.samplerCueGotoAndPlay ? "cue_gotoandplay" : "start_play";

    if (engine.getValue(sampleDeck, "track_loaded")) {
        engine.setValue(sampleDeck, playMode, value ? 1 : 0);
    } else {
        engine.setValue(sampleDeck, "LoadSelectedTrack", value ? 1 : 0);
    }
};

PioneerDDJSX3.stopSamplerButtons = function(channel, control, value, status, group) {
    var index = control - 0x38 + 1,
        deckOffset = PioneerDDJSX3.selectedSamplerBank * 8,
        sampleDeck = "[Sampler" + (index + deckOffset) + "]",
        trackLoaded = engine.getValue(sampleDeck, "track_loaded"),
        playing = engine.getValue(sampleDeck, "play");

    if (trackLoaded && playing) {
        script.toggleControl(sampleDeck, "stop");
    } else if (trackLoaded && !playing && value) {
        script.toggleControl(sampleDeck, "eject");
    }
};

PioneerDDJSX3.samplerVelocityVolume = function(channel, control, value, status, group) {
    var index = control - 0x30 + 1,
        deck = PioneerDDJSX3.channelGroups[group],
        deckOffset = PioneerDDJSX3.selectedSamplerBank * 8,
        sampleDeck = "[Sampler" + (index + deckOffset) + "]",
        vol = value / 0x7F;

    if (PioneerDDJSX3.samplerVelocityMode[deck]) {
        engine.setParameter(sampleDeck, "volume", vol);
    }
};

PioneerDDJSX3.changeParameters = function(group, ctrl, value) {
    var deck = PioneerDDJSX3.channelGroups[group],
        index,
        offset = 0,
        samplerIndex = 0,
        beatjumpSize = 0;

    //Hot Cue Mode:
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftHotCueMode) {
        engine.setValue(group, "beatjump_backward", value);
    }
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterRightHotCueMode) {
        engine.setValue(group, "beatjump_forward", value);
    }
    if (ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterLeftHotCueMode) {
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterLeftHotCueMode, value);
        if (value) {
            beatjumpSize = engine.getValue(group, "beatjump_size");
            engine.setValue(group, "beatjump_size", beatjumpSize / 2);
        }
    }
    if (ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterRightHotCueMode) {
        PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterRightHotCueMode, value);
        if (value) {
            beatjumpSize = engine.getValue(group, "beatjump_size");
            engine.setValue(group, "beatjump_size", beatjumpSize * 2);
        }
    }

    // ignore other cases if button is released:
    if (!value) {
        return;
    }

    //Roll Mode:
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftRollMode || ctrl === PioneerDDJSX3.nonPadLeds.parameterRightRollMode) {
        // unbind previous connected controls:
        for (index in PioneerDDJSX3.selectedLooprollIntervals[deck]) {
            if (PioneerDDJSX3.selectedLooprollIntervals[deck].hasOwnProperty(index)) {
                engine.connectControl(
                    group,
                    "beatlooproll_" + PioneerDDJSX3.selectedLooprollIntervals[deck][index] + "_activate",
                    "PioneerDDJSX3.beatlooprollLeds",
                    true
                );
            }
        }
        // change parameter set:
        if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftRollMode && PioneerDDJSX3.selectedLoopRollParam[deck] > 0) {
            PioneerDDJSX3.selectedLoopRollParam[deck] -= 1;
        } else if (ctrl === PioneerDDJSX3.nonPadLeds.parameterRightRollMode && PioneerDDJSX3.selectedLoopRollParam[deck] < 3) {
            PioneerDDJSX3.selectedLoopRollParam[deck] += 1;
        }
        PioneerDDJSX3.selectedLooprollIntervals[deck] = PioneerDDJSX3.loopIntervals[PioneerDDJSX3.selectedLoopRollParam[deck]];
        // bind new controls:
        for (index in PioneerDDJSX3.selectedLooprollIntervals[deck]) {
            if (PioneerDDJSX3.selectedLooprollIntervals[deck].hasOwnProperty(index)) {
                engine.connectControl(
                    group,
                    "beatlooproll_" + PioneerDDJSX3.selectedLooprollIntervals[deck][index] + "_activate",
                    "PioneerDDJSX3.beatlooprollLeds",
                    false
                );
            }
        }
    }

    //Group2 (Beatloop) Mode:
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftGroup2Mode || ctrl === PioneerDDJSX3.nonPadLeds.parameterRightGroup2Mode) {
        // unbind previous connected controls:
        for (index in PioneerDDJSX3.selectedLoopIntervals[deck]) {
            if (PioneerDDJSX3.selectedLoopIntervals[deck].hasOwnProperty(index)) {
                engine.connectControl(
                    group,
                    "beatloop_" + PioneerDDJSX3.selectedLoopIntervals[deck][index] + "_enabled",
                    "PioneerDDJSX3.beatloopLeds",
                    true
                );
            }
        }
        // change parameter set:
        if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftGroup2Mode && PioneerDDJSX3.selectedLoopParam[deck] > 0) {
            PioneerDDJSX3.selectedLoopParam[deck] -= 1;
        } else if (ctrl === PioneerDDJSX3.nonPadLeds.parameterRightGroup2Mode && PioneerDDJSX3.selectedLoopParam[deck] < 3) {
            PioneerDDJSX3.selectedLoopParam[deck] += 1;
        }
        PioneerDDJSX3.selectedLoopIntervals[deck] = PioneerDDJSX3.loopIntervals[PioneerDDJSX3.selectedLoopParam[deck]];
        // bind new controls:
        for (index in PioneerDDJSX3.selectedLoopIntervals[deck]) {
            if (PioneerDDJSX3.selectedLoopIntervals[deck].hasOwnProperty(index)) {
                engine.connectControl(
                    group,
                    "beatloop_" + PioneerDDJSX3.selectedLoopIntervals[deck][index] + "_enabled",
                    "PioneerDDJSX3.beatloopLeds",
                    false
                );
            }
        }
    }

    //Sampler Mode:
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftSamplerMode || ctrl === PioneerDDJSX3.nonPadLeds.parameterRightSamplerMode) {
        // unbind previous connected controls:
        for (index in PioneerDDJSX3.samplerGroups) {
            if (PioneerDDJSX3.samplerGroups.hasOwnProperty(index)) {
                offset = PioneerDDJSX3.selectedSamplerBank * 8;
                samplerIndex = (PioneerDDJSX3.samplerGroups[index] + 1) + offset;
                engine.connectControl(
                    "[Sampler" + samplerIndex + "]",
                    "duration",
                    "PioneerDDJSX3.samplerLeds",
                    true
                );
                engine.connectControl(
                    "[Sampler" + samplerIndex + "]",
                    "play",
                    "PioneerDDJSX3.samplerLedsPlay",
                    true
                );
            }
        }
        // change sampler bank:
        if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftSamplerMode && PioneerDDJSX3.selectedSamplerBank > 0) {
            PioneerDDJSX3.selectedSamplerBank -= 1;
        } else if (ctrl === PioneerDDJSX3.nonPadLeds.parameterRightSamplerMode && PioneerDDJSX3.selectedSamplerBank < 3) {
            PioneerDDJSX3.selectedSamplerBank += 1;
        }
        // bind new controls:
        for (index in PioneerDDJSX3.samplerGroups) {
            if (PioneerDDJSX3.samplerGroups.hasOwnProperty(index)) {
                offset = PioneerDDJSX3.selectedSamplerBank * 8;
                samplerIndex = (PioneerDDJSX3.samplerGroups[index] + 1) + offset;
                engine.connectControl(
                    "[Sampler" + samplerIndex + "]",
                    "duration",
                    "PioneerDDJSX3.samplerLeds",
                    false
                );
                engine.connectControl(
                    "[Sampler" + samplerIndex + "]",
                    "play",
                    "PioneerDDJSX3.samplerLedsPlay",
                    false
                );
                engine.trigger("[Sampler" + samplerIndex + "]", "duration");
            }
        }
    }

    //Slicer Mode:
    if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftSlicerMode || ctrl === PioneerDDJSX3.nonPadLeds.parameterRightSlicerMode) {
        // change parameter set:
        if (ctrl === PioneerDDJSX3.nonPadLeds.parameterLeftSlicerMode && PioneerDDJSX3.selectedSlicerQuantizeParam[deck] > 0) {
            PioneerDDJSX3.selectedSlicerQuantizeParam[deck] -= 1;
        } else if (ctrl === PioneerDDJSX3.nonPadLeds.parameterRightSlicerMode && PioneerDDJSX3.selectedSlicerQuantizeParam[deck] < 3) {
            PioneerDDJSX3.selectedSlicerQuantizeParam[deck] += 1;
        }
        PioneerDDJSX3.selectedSlicerQuantization[deck] = PioneerDDJSX3.slicerQuantizations[PioneerDDJSX3.selectedSlicerQuantizeParam[deck]];
    }
    //Slicer Mode + SHIFT:
    if (ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterLeftSlicerMode || ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterRightSlicerMode) {
        // change parameter set:
        if (ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterLeftSlicerMode && PioneerDDJSX3.selectedSlicerDomainParam[deck] > 0) {
            PioneerDDJSX3.selectedSlicerDomainParam[deck] -= 1;
        } else if (ctrl === PioneerDDJSX3.nonPadLeds.shiftParameterRightSlicerMode && PioneerDDJSX3.selectedSlicerDomainParam[deck] < 3) {
            PioneerDDJSX3.selectedSlicerDomainParam[deck] += 1;
        }
        PioneerDDJSX3.selectedSlicerDomain[deck] = PioneerDDJSX3.slicerDomains[PioneerDDJSX3.selectedSlicerDomainParam[deck]];
    }

    // update parameter status leds:
    PioneerDDJSX3.updateParameterStatusLeds(
        group,
        PioneerDDJSX3.selectedLoopRollParam[deck],
        PioneerDDJSX3.selectedLoopParam[deck],
        PioneerDDJSX3.selectedSamplerBank,
        PioneerDDJSX3.selectedSlicerQuantizeParam[deck],
        PioneerDDJSX3.selectedSlicerDomainParam[deck]
    );
};

PioneerDDJSX3.parameterLeft = function(channel, control, value, status, group) {
    PioneerDDJSX3.changeParameters(group, control, value);
};

PioneerDDJSX3.parameterRight = function(channel, control, value, status, group) {
    PioneerDDJSX3.changeParameters(group, control, value);
};

PioneerDDJSX3.shiftParameterLeft = function(channel, control, value, status, group) {
    PioneerDDJSX3.changeParameters(group, control, value);
};

PioneerDDJSX3.shiftParameterRight = function(channel, control, value, status, group) {
    PioneerDDJSX3.changeParameters(group, control, value);
};

PioneerDDJSX3.vinylButton = function(channel, control, value, status, group) {
    PioneerDDJSX3.toggleScratch(channel, control, value, status, group);
};

PioneerDDJSX3.slipButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "slip_enabled");
    }
};

PioneerDDJSX3.keyLockButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "keylock");
    }
};

PioneerDDJSX3.shiftKeyLockButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group],
        range = engine.getValue(group, "rateRange");

    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftKeyLock, value);

    if (range === 0.90) {
        range = PioneerDDJSX3.setUpSpeedSliderRange[deck];
    } else if ((range * 2) > 0.90) {
        range = 0.90;
    } else {
        range = range * 2;
    }

    if (value) {
        engine.setValue(group, "rateRange", range);
    }
};

PioneerDDJSX3.tempoResetButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    if (value) {
        engine.setValue(group, "rate", 0);
        if (PioneerDDJSX3.syncRate[deck] !== engine.getValue(group, "rate")) {
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, 0);
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, 0);
            PioneerDDJSX3.syncRate[deck] = 0;
        }
    }
};

PioneerDDJSX3.autoLoopButton = function(channel, control, value, status, group) {
    if (value) {
        if (engine.getValue(group, "loop_enabled")) {
            engine.setValue(group, "reloop_toggle", true);
            engine.setValue(group, "reloop_toggle", false);
        } else {
            engine.setValue(group, "beatloop_activate", true);
            engine.setValue(group, "beatloop_activate", false);
        }
    }
};

PioneerDDJSX3.loopActiveButton = function(channel, control, value, status, group) {
    engine.setValue(group, "reloop_toggle", value);
};

PioneerDDJSX3.loopInButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "loop_in");
};

PioneerDDJSX3.shiftLoopInButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "reloop_andstop");
};

PioneerDDJSX3.loopOutButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "loop_out");
};

PioneerDDJSX3.loopExitButton = function(channel, control, value, status, group) {
    engine.setValue(group, "reloop_toggle", value);
};

PioneerDDJSX3.loopHalveButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "loop_halve");
};

PioneerDDJSX3.loopDoubleButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "loop_double");
};

PioneerDDJSX3.loopMoveBackButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "beatjump_1_backward");
};

PioneerDDJSX3.loopMoveForwardButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "beatjump_1_forward");
};

PioneerDDJSX3.loadButton = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "LoadSelectedTrack", true);
        if (PioneerDDJSX3.autoPFL) {
            for (var index in PioneerDDJSX3.channelGroups) {
                if (PioneerDDJSX3.channelGroups.hasOwnProperty(index)) {
                    if (index === group) {
                        engine.setValue(index, "pfl", true);
                    } else {
                        engine.setValue(index, "pfl", false);
                    }
                }
            }
        }
    }
};

PioneerDDJSX3.crossfaderAssignCenter = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "orientation", 1);
    }
};

PioneerDDJSX3.crossfaderAssignLeft = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "orientation", 0);
    }
};

PioneerDDJSX3.crossfaderAssignRight = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "orientation", 2);
    }
};

PioneerDDJSX3.reverseRollButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "reverseroll");
};

PioneerDDJSX3.reverseButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "reverse");
};

PioneerDDJSX3.gridAdjustButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    PioneerDDJSX3.gridAdjustSelected[deck] = value ? true : false;
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.gridAdjust, value);
};

PioneerDDJSX3.gridSetButton = function(channel, control, value, status, group) {
    script.toggleControl(group, "beats_translate_curpos");
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftGridAdjust, value);
};

PioneerDDJSX3.gridSlideButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    PioneerDDJSX3.gridSlideSelected[deck] = value ? true : false;
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.gridSlide, value);
};

PioneerDDJSX3.syncButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerDDJSX3.quantizeButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "quantize");
    }
};

PioneerDDJSX3.needleSearchTouch = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    if (engine.getValue(group, "play")) {
        PioneerDDJSX3.needleSearchTouched[deck] = PioneerDDJSX3.shiftPressed && (value ? true : false);
    } else {
        PioneerDDJSX3.needleSearchTouched[deck] = value ? true : false;
    }
};

PioneerDDJSX3.needleSearchStripPosition = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    if (PioneerDDJSX3.needleSearchTouched[deck]) {
        var position = value / 0x7F;
        engine.setValue(group, "playposition", position);
    }
};

PioneerDDJSX3.panelSelectButton = function(channel, control, value, status, group) {
    if (value) {
        if ((PioneerDDJSX3.panels[0] === false) && (PioneerDDJSX3.panels[1] === false)) {
            PioneerDDJSX3.panels[0] = true;
        } else if ((PioneerDDJSX3.panels[0] === true) && (PioneerDDJSX3.panels[1] === false)) {
            PioneerDDJSX3.panels[1] = true;
        } else if ((PioneerDDJSX3.panels[0] === true) && (PioneerDDJSX3.panels[1] === true)) {
            PioneerDDJSX3.panels[0] = false;
        } else if ((PioneerDDJSX3.panels[0] === false) && (PioneerDDJSX3.panels[1] === true)) {
            PioneerDDJSX3.panels[1] = false;
        }

        engine.setValue("[Samplers]", "show_samplers", PioneerDDJSX3.panels[0]);
        engine.setValue("[EffectRack1]", "show", PioneerDDJSX3.panels[1]);
    }
};

PioneerDDJSX3.shiftPanelSelectButton = function(channel, control, value, status, group) {
    var channelGroup;
    PioneerDDJSX3.shiftPanelSelectPressed = value ? true : false;
    
    for (var index in PioneerDDJSX3.fxUnitGroups) {
        if (PioneerDDJSX3.fxUnitGroups.hasOwnProperty(index)) {
            if (PioneerDDJSX3.fxUnitGroups[index] < 2) {
                for (channelGroup in PioneerDDJSX3.channelGroups) {
                    if (PioneerDDJSX3.channelGroups.hasOwnProperty(channelGroup)) {
                        engine.connectControl(index, "group_" + channelGroup + "_enable", "PioneerDDJSX3.fxAssignLeds", value);
                        if (value) {
                            engine.trigger(index, "group_" + channelGroup + "_enable");
                        }
                    }
                }
            }
            if (PioneerDDJSX3.fxUnitGroups[index] >= 2) {
                for (channelGroup in PioneerDDJSX3.channelGroups) {
                    if (PioneerDDJSX3.channelGroups.hasOwnProperty(channelGroup)) {
                        engine.connectControl(index, "group_" + channelGroup + "_enable", "PioneerDDJSX3.fxAssignLeds", !value);
                        if (value) {
                            engine.trigger(index, "group_" + channelGroup + "_enable");
                        } else {
                            PioneerDDJSX3.fxAssignLedControl(index, PioneerDDJSX3.channelGroups[channelGroup], false);
                        }
                    }
                }
            }
        }
    }
};


///////////////////////////////////////////////////////////////
//                          LED HELPERS                      //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.deckConverter = function(group) {
    if (PioneerDDJSX3.channelGroups.hasOwnProperty(group)) {
        return PioneerDDJSX3.channelGroups[group];
    }
    return group;
};

PioneerDDJSX3.flashLedState = 0;

PioneerDDJSX3.flashLed = function(deck, ledNumber) {
    if (PioneerDDJSX3.flashLedState === 0) {
        PioneerDDJSX3.nonPadLedControl(deck, ledNumber, 1);
        PioneerDDJSX3.flashLedState = 1;
    } else if (PioneerDDJSX3.flashLedState === 1) {
        PioneerDDJSX3.nonPadLedControl(deck, ledNumber, 0);
        PioneerDDJSX3.flashLedState = 0;
    }
};

PioneerDDJSX3.resetNonDeckLeds = function() {
    var indexFxUnit;

    // fx Leds:
    for (indexFxUnit in PioneerDDJSX3.fxUnitGroups) {
        if (PioneerDDJSX3.fxUnitGroups.hasOwnProperty(indexFxUnit)) {
            if (PioneerDDJSX3.fxUnitGroups[indexFxUnit] < 2) {
                for (var indexFxLed in PioneerDDJSX3.fxEffectGroups) {
                    if (PioneerDDJSX3.fxEffectGroups.hasOwnProperty(indexFxLed)) {
                        PioneerDDJSX3.fxLedControl(
                            PioneerDDJSX3.fxUnitGroups[indexFxUnit],
                            PioneerDDJSX3.fxEffectGroups[indexFxLed],
                            false,
                            false
                        );
                        PioneerDDJSX3.fxLedControl(
                            PioneerDDJSX3.fxUnitGroups[indexFxUnit],
                            PioneerDDJSX3.fxEffectGroups[indexFxLed],
                            true,
                            false
                        );
                    }
                }
                PioneerDDJSX3.fxLedControl(PioneerDDJSX3.fxUnitGroups[indexFxUnit], 0x03, false, false);
                PioneerDDJSX3.fxLedControl(PioneerDDJSX3.fxUnitGroups[indexFxUnit], 0x03, true, false);
            }
        }
    }

    // fx assign Leds:
    for (indexFxUnit in PioneerDDJSX3.fxUnitGroups) {
        if (PioneerDDJSX3.fxUnitGroups.hasOwnProperty(indexFxUnit)) {
            for (var channelGroup in PioneerDDJSX3.channelGroups) {
                if (PioneerDDJSX3.channelGroups.hasOwnProperty(channelGroup)) {
                    PioneerDDJSX3.fxAssignLedControl(
                        indexFxUnit,
                        PioneerDDJSX3.channelGroups[channelGroup],
                        false
                    );
                }
            }
        }
    }

    // general Leds:
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftMasterCue, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.loadDeck1, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck1, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.loadDeck2, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck2, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.loadDeck3, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck3, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.loadDeck4, false);
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftLoadDeck4, false);
};

PioneerDDJSX3.fxAssignLedControl = function(unit, ledNumber, active) {
    var fxAssignLedsBaseChannel = 0x96,
        fxAssignLedsBaseControl = 0;

    if (unit === "[EffectRack1_EffectUnit1]") {
        fxAssignLedsBaseControl = PioneerDDJSX3.nonPadLeds.fx1assignDeck1;
    }
    if (unit === "[EffectRack1_EffectUnit2]") {
        fxAssignLedsBaseControl = PioneerDDJSX3.nonPadLeds.fx2assignDeck1;
    }

    midi.sendShortMsg(
        fxAssignLedsBaseChannel,
        fxAssignLedsBaseControl + ledNumber,
        active ? 0x7F : 0x00
    );
};

PioneerDDJSX3.fxLedControl = function(unit, ledNumber, shift, active) {
    var fxLedsBaseChannel = 0x94,
        fxLedsBaseControl = (shift ? 0x63 : 0x47);

    midi.sendShortMsg(
        fxLedsBaseChannel + unit,
        fxLedsBaseControl + ledNumber,
        active ? 0x7F : 0x00
    );
};

PioneerDDJSX3.padLedControl = function(deck, groupNumber, ledNumber, shift, active) {
    var padLedsBaseChannel = 0x97,
        padLedControl = (shift ? 0x08 : 0x00) + groupNumber + ledNumber,
        midiChannelOffset = PioneerDDJSX3.deckConverter(deck);

    if (midiChannelOffset !== null) {
        midi.sendShortMsg(
            padLedsBaseChannel + midiChannelOffset,
            padLedControl,
            active ? 0x7F : 0x00
        );
    }
};

PioneerDDJSX3.nonPadLedControl = function(deck, ledNumber, active) {
    var nonPadLedsBaseChannel = 0x90,
        midiChannelOffset = PioneerDDJSX3.deckConverter(deck);

    if (midiChannelOffset !== null) {
        midi.sendShortMsg(
            nonPadLedsBaseChannel + midiChannelOffset,
            ledNumber,
            active ? 0x7F : 0x00
        );
    }
};

PioneerDDJSX3.illuminateFunctionControl = function(ledNumber, active) {
    var illuminationBaseChannel = 0x9B;

    midi.sendShortMsg(
        illuminationBaseChannel,
        ledNumber,
        active ? 0x7F : 0x00
    );
};

PioneerDDJSX3.wheelLedControl = function(deck, ledNumber) {
    var wheelLedBaseChannel = 0xBB,
        channel = PioneerDDJSX3.deckConverter(deck);

    if (channel !== null) {
        midi.sendShortMsg(
            wheelLedBaseChannel,
            channel,
            ledNumber
        );
    }
};

PioneerDDJSX3.generalLedControl = function(ledNumber, active) {
    var generalLedBaseChannel = 0x96;

    midi.sendShortMsg(
        generalLedBaseChannel,
        ledNumber,
        active ? 0x7F : 0x00
    );
};

PioneerDDJSX3.updateParameterStatusLeds = function(group, statusRoll, statusLoop, statusSampler, statusSlicerQuant, statusSlicerDomain) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftRollMode, statusRoll & (1 << 1));
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightRollMode, statusRoll & 1);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftGroup2Mode, statusLoop & (1 << 1));
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightGroup2Mode, statusLoop & 1);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftSamplerMode, statusSampler & (1 << 1));
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightSamplerMode, statusSampler & 1);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftSlicerMode, statusSlicerQuant & (1 << 1));
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightSlicerMode, statusSlicerQuant & 1);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterLeftSlicerMode, statusSlicerDomain & (1 << 1));
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftParameterRightSlicerMode, statusSlicerDomain & 1);
};


///////////////////////////////////////////////////////////////
//                             LEDS                          //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.fxAssignLeds = function(value, group, control) {
    var channelGroup = control.replace("group_", '').replace("_enable", '');
    PioneerDDJSX3.fxAssignLedControl(group, PioneerDDJSX3.channelGroups[channelGroup], value);
};

PioneerDDJSX3.headphoneCueLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.headphoneCue, value);
};

PioneerDDJSX3.shiftHeadphoneCueLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftHeadphoneCue, value);
};

PioneerDDJSX3.shiftMasterCueLed = function(value, group, control) {
    PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds.shiftMasterCue, value);
};

PioneerDDJSX3.keyLockLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.keyLock, value);
};

PioneerDDJSX3.playLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.play, value);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftPlay, value);
};

PioneerDDJSX3.wheelLeds = function(value, group, control) {
    // Timing calculation is handled in seconds!
    var deck = PioneerDDJSX3.channelGroups[group],
        duration = engine.getValue(group, "duration"),
        elapsedTime = value * duration,
        remainingTime = duration - elapsedTime,
        revolutionsPerSecond = PioneerDDJSX3.scratchSettings.vinylSpeed / 60,
        speed = parseInt(revolutionsPerSecond * PioneerDDJSX3.wheelLedCircle.maxVal),
        wheelPos = PioneerDDJSX3.wheelLedCircle.minVal;

    if (value >= 0) {
        wheelPos = PioneerDDJSX3.wheelLedCircle.minVal + 0x01 + ((speed * elapsedTime) % PioneerDDJSX3.wheelLedCircle.maxVal);
    } else {
        wheelPos = PioneerDDJSX3.wheelLedCircle.maxVal + 0x01 + ((speed * elapsedTime) % PioneerDDJSX3.wheelLedCircle.maxVal);
    }
/*
 *
    // let wheel LEDs blink if remaining time is less than 30s:
    if (remainingTime > 0 && remainingTime < 30 && !engine.isScratching(deck + 1)) {
        var blinkInterval = parseInt(remainingTime / 3); //increase blinking according time left
        if (blinkInterval < 3) {
            blinkInterval = 3;
        }
        if (PioneerDDJSX3.wheelLedsBlinkStatus[deck] < blinkInterval) {
            wheelPos = PioneerDDJSX3.wheelLedCircle.minVal;
        } else if (PioneerDDJSX3.wheelLedsBlinkStatus[deck] > (blinkInterval - parseInt(6 / blinkInterval))) {
            PioneerDDJSX3.wheelLedsBlinkStatus[deck] = 0;
        }
        PioneerDDJSX3.wheelLedsBlinkStatus[deck]++;
    }
    */
    PioneerDDJSX3.wheelLedControl(group, wheelPos);
};

PioneerDDJSX3.cueLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.cue, value);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftCue, value);
};

PioneerDDJSX3.loadLed = function(value, group, control) {
    var deck = PioneerDDJSX3.channelGroups[group];
    if (value > 0) {
        PioneerDDJSX3.wheelLedControl(group, PioneerDDJSX3.wheelLedCircle.maxVal);
        PioneerDDJSX3.generalLedControl(PioneerDDJSX3.nonPadLeds["loadDeck" + (deck + 1)], true);
        PioneerDDJSX3.illuminateFunctionControl(PioneerDDJSX3.illuminationControl["loadedDeck" + (deck + 1)], true);
        engine.trigger(group, "playposition");
    } else {
        PioneerDDJSX3.wheelLedControl(group, PioneerDDJSX3.wheelLedCircle.minVal);
    }
};

PioneerDDJSX3.reverseLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.censor, value);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftCensor, value);
};

PioneerDDJSX3.slipLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.slip, value);
};

PioneerDDJSX3.quantizeLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftSync, value);
};

PioneerDDJSX3.syncLed = function(value, group, control) {
    var deck = PioneerDDJSX3.channelGroups[group];
    var rate = engine.getValue(group, "rate");
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.sync, value);
    if (value) {
        PioneerDDJSX3.syncRate[deck] = rate;
        if (PioneerDDJSX3.syncRate[deck] > 0) {
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, 1);
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, 0);
        } else if (PioneerDDJSX3.syncRate[deck] < 0) {
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, 0);
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, 1);
        }
    }
    if (!value) {
        if (PioneerDDJSX3.syncRate[deck] !== rate || rate === 0) {
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverPlus, 0);
            PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.takeoverMinus, 0);
            PioneerDDJSX3.syncRate[deck] = 0;
        }
    }
};

PioneerDDJSX3.autoLoopLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.autoLoop, value);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopOut, value);
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftAutoLoop, value);
};

PioneerDDJSX3.loopInLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopIn, value);
};

PioneerDDJSX3.shiftLoopInLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopIn, value);
};

PioneerDDJSX3.loopOutLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopOut, value);
};

PioneerDDJSX3.loopHalveLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopHalve, value);
};

PioneerDDJSX3.loopDoubleLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.loopDouble, value);
};

PioneerDDJSX3.loopShiftFWLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopDouble, value);
};

PioneerDDJSX3.loopShiftBKWLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.shiftLoopHalve, value);
};

PioneerDDJSX3.hotCueParameterRightLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterRightHotCueMode, value);
};

PioneerDDJSX3.hotCueParameterLeftLed = function(value, group, control) {
    PioneerDDJSX3.nonPadLedControl(group, PioneerDDJSX3.nonPadLeds.parameterLeftHotCueMode, value);
};

PioneerDDJSX3.samplerLeds = function(value, group, control) {
    var samplerIndex = (group.replace("[Sampler", '').replace(']', '') - 1) % 8,
        sampleDeck = "[Sampler" + (samplerIndex + 1) + "]",
        padNum = PioneerDDJSX3.samplerGroups[sampleDeck];

    for (var index in PioneerDDJSX3.channelGroups) {
        if (PioneerDDJSX3.channelGroups.hasOwnProperty(index)) {
            PioneerDDJSX3.padLedControl(
                PioneerDDJSX3.channelGroups[index],
                PioneerDDJSX3.ledGroups.sampler,
                padNum,
                false,
                value
            );
        }
    }
};

PioneerDDJSX3.samplerLedsPlay = function(value, group, control) {
    var samplerIndex = (group.replace("[Sampler", '').replace(']', '') - 1) % 8,
        sampleDeck = "[Sampler" + (samplerIndex + 1) + "]",
        padNum = PioneerDDJSX3.samplerGroups[sampleDeck];

    if (!engine.getValue(sampleDeck, "duration")) {
        return;
    }

    for (var index in PioneerDDJSX3.channelGroups) {
        if (PioneerDDJSX3.channelGroups.hasOwnProperty(index)) {
            PioneerDDJSX3.padLedControl(
                PioneerDDJSX3.channelGroups[index],
                PioneerDDJSX3.ledGroups.sampler,
                padNum,
                false, !value
            );
            PioneerDDJSX3.padLedControl(
                PioneerDDJSX3.channelGroups[index],
                PioneerDDJSX3.ledGroups.sampler,
                padNum,
                true,
                value
            );
        }
    }
};

PioneerDDJSX3.beatloopLeds = function(value, group, control) {
    var padNum,
        shifted = false,
        deck = PioneerDDJSX3.channelGroups[group];

    for (var index in PioneerDDJSX3.selectedLoopIntervals[deck]) {
        if (PioneerDDJSX3.selectedLoopIntervals[deck].hasOwnProperty(index)) {
            if (control === "beatloop_" + PioneerDDJSX3.selectedLoopIntervals[deck][index] + "_enabled") {
                padNum = index % 8;
                PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.group2, padNum, shifted, value);
            }
        }
    }
};

PioneerDDJSX3.beatlooprollLeds = function(value, group, control) {
    var padNum,
        shifted = false,
        deck = PioneerDDJSX3.channelGroups[group];

    for (var index in PioneerDDJSX3.selectedLooprollIntervals[deck]) {
        if (PioneerDDJSX3.selectedLooprollIntervals[deck].hasOwnProperty(index)) {
            if (control === "beatlooproll_" + PioneerDDJSX3.selectedLooprollIntervals[deck][index] + "_activate") {
                padNum = index % 8;
                PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.loopRoll, padNum, shifted, value);
            }
        }
    }
};

PioneerDDJSX3.hotCueLeds = function(value, group, control) {
    var padNum = null,
        hotCueNum;

    for (hotCueNum = 1; hotCueNum <= 8; hotCueNum++) {
        if (control === "hotcue_" + hotCueNum + "_enabled") {
            padNum = (hotCueNum - 1);
            PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.hotCue, padNum, false, value);
            PioneerDDJSX3.padLedControl(group, PioneerDDJSX3.ledGroups.hotCue, padNum, true, value);
        }
    }
};

PioneerDDJSX3.VuMeterLeds = function(value, group, control) {
    // Remark: Only deck vu meters can be controlled! Master vu meter is handled by hardware!
    var midiBaseAdress = 0xB0,
        channel = 0x02,
        midiOut = 0x00;

    value = parseInt(value * 0x76); //full level indicator: 0x7F

    if (engine.getValue(group, "PeakIndicator")) {
        value = value + 0x09;
    }

    PioneerDDJSX3.valueVuMeter[group + "_current"] = value;

    for (var index in PioneerDDJSX3.channelGroups) {
        if (PioneerDDJSX3.channelGroups.hasOwnProperty(index)) {
            midiOut = PioneerDDJSX3.valueVuMeter[index + "_current"];
            if (PioneerDDJSX3.twinkleVumeterAutodjOn) {
                if (engine.getValue("[AutoDJ]", "enabled")) {
                    if (PioneerDDJSX3.valueVuMeter[index + "_enabled"]) {
                        midiOut = 0;
                    }
                    if (midiOut < 5 && !PioneerDDJSX3.valueVuMeter[index + "_enabled"]) {
                        midiOut = 5;
                    }
                }
            }
	    print("vumeter",channel,midiOut);
            midi.sendShortMsg(
                midiBaseAdress + PioneerDDJSX3.channelGroups[index],
                channel,
                midiOut
            );
        }
    }
};


///////////////////////////////////////////////////////////////
//                          JOGWHEELS                        //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.getJogWheelDelta = function(value) {
    // The Wheel control centers on 0x40; find out how much it's moved by.
    return value - 0x40;
};

PioneerDDJSX3.jogRingTick = function(channel, control, value, status, group) {
    PioneerDDJSX3.pitchBendFromJog(group, PioneerDDJSX3.getJogWheelDelta(value));
};

PioneerDDJSX3.jogRingTickShift = function(channel, control, value, status, group) {
    PioneerDDJSX3.pitchBendFromJog(
        group,
        PioneerDDJSX3.getJogWheelDelta(value) * PioneerDDJSX3.jogwheelShiftMultiplier
    );
};

PioneerDDJSX3.jogPlatterTick = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    if (PioneerDDJSX3.gridAdjustSelected[deck]) {
        if (PioneerDDJSX3.getJogWheelDelta(value) > 0) {
            script.toggleControl(group, "beats_adjust_faster");
        }
        if (PioneerDDJSX3.getJogWheelDelta(value) <= 0) {
            script.toggleControl(group, "beats_adjust_slower");
        }
        return;
    }
    if (PioneerDDJSX3.gridSlideSelected[deck]) {
        if (PioneerDDJSX3.getJogWheelDelta(value) > 0) {
            script.toggleControl(group, "beats_translate_later");
        }
        if (PioneerDDJSX3.getJogWheelDelta(value) <= 0) {
            script.toggleControl(group, "beats_translate_earlier");
        }
        return;
    }

    if (PioneerDDJSX3.scratchMode[deck] && engine.isScratching(deck + 1)) {
        engine.scratchTick(deck + 1, PioneerDDJSX3.getJogWheelDelta(value));
    } else {
        PioneerDDJSX3.pitchBendFromJog(group, PioneerDDJSX3.getJogWheelDelta(value));
    }
};

PioneerDDJSX3.jogPlatterTickShift = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    if (PioneerDDJSX3.scratchMode[deck] && engine.isScratching(deck + 1)) {
        engine.scratchTick(deck + 1, PioneerDDJSX3.getJogWheelDelta(value));
    } else {
        PioneerDDJSX3.pitchBendFromJog(
            group,
            PioneerDDJSX3.getJogWheelDelta(value) * PioneerDDJSX3.jogwheelShiftMultiplier
        );
    }
};

PioneerDDJSX3.jogTouch = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];

    if (PioneerDDJSX3.scratchMode[deck]) {
        if (value) {
            engine.scratchEnable(
                deck + 1,
                PioneerDDJSX3.scratchSettings.jogResolution,
                PioneerDDJSX3.scratchSettings.vinylSpeed,
                PioneerDDJSX3.scratchSettings.alpha,
                PioneerDDJSX3.scratchSettings.beta,
                true
            );
        } else {
            engine.scratchDisable(deck + 1, true);
        }
    }
};

PioneerDDJSX3.toggleScratch = function(channel, control, value, status, group) {
    var deck = PioneerDDJSX3.channelGroups[group];
    if (value) {
        PioneerDDJSX3.scratchMode[deck] = !PioneerDDJSX3.scratchMode[deck];
        PioneerDDJSX3.triggerVinylLed(deck);
    }
};

PioneerDDJSX3.triggerVinylLed = function(deck) {
    PioneerDDJSX3.nonPadLedControl(deck, PioneerDDJSX3.nonPadLeds.vinyl, PioneerDDJSX3.scratchMode[deck]);
};

PioneerDDJSX3.pitchBendFromJog = function(group, movement) {
    engine.setValue(group, "jog", movement / 5 * PioneerDDJSX3.jogwheelSensivity);
};


///////////////////////////////////////////////////////////////
//             ROTARY SELECTOR & NAVIGATION BUTTONS          //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.loadPrepareButton = function(channel, control, value, status) {
    if (PioneerDDJSX3.rotarySelectorChanged === true) {
        if (value) {
            engine.setValue("[PreviewDeck1]", "LoadSelectedTrackAndPlay", true);
        } else {
            if (PioneerDDJSX3.jumpPreviewEnabled) {
                engine.setValue("[PreviewDeck1]", "playposition", PioneerDDJSX3.jumpPreviewPosition);
            }
            PioneerDDJSX3.rotarySelectorChanged = false;
        }
    } else {
        if (value) {
            if (engine.getValue("[PreviewDeck1]", "stop")) {
                script.toggleControl("[PreviewDeck1]", "play");
            } else {
                script.toggleControl("[PreviewDeck1]", "stop");
            }
        }
    }
};

PioneerDDJSX3.backButton = function(channel, control, value, status) {
    script.toggleControl("[Library]", "MoveFocusBackward");
};

PioneerDDJSX3.shiftBackButton = function(channel, control, value, status) {
    if (value) {
        script.toggleControl("[Master]", "maximize_library");
    }
};

PioneerDDJSX3.getRotaryDelta = function(value) {
    var delta = 0x40 - Math.abs(0x40 - value),
        isCounterClockwise = value > 0x40;

    if (isCounterClockwise) {
        delta *= -1;
    }
    return delta;
};

PioneerDDJSX3.rotarySelector = function(channel, control, value, status) {
    var delta = PioneerDDJSX3.getRotaryDelta(value);

    engine.setValue("[Library]", "MoveVertical", delta);
    PioneerDDJSX3.rotarySelectorChanged = true;
};

PioneerDDJSX3.rotarySelectorShifted = function(channel, control, value, status) {
    var delta = PioneerDDJSX3.getRotaryDelta(value),
        f = (delta > 0 ? "SelectNextPlaylist" : "SelectPrevPlaylist");

    engine.setValue("[Library]", "MoveHorizontal", delta);
};

PioneerDDJSX3.rotarySelectorClick = function(channel, control, value, status) {
    script.toggleControl("[Library]", "GoToItem");
};

PioneerDDJSX3.rotarySelectorShiftedClick = function(channel, control, value, status) {
    if (PioneerDDJSX3.autoDJAddTop) {
        script.toggleControl("[Library]", "AutoDjAddTop");
    } else {
        script.toggleControl("[Library]", "AutoDjAddBottom");
    }
};


///////////////////////////////////////////////////////////////
//                             FX                            //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.fxAssignButton = function(channel, control, value, status, group) {
    if (value) {
        if ((control >= 0x50) && (control <= 0x53)) {
            script.toggleControl("[EffectRack1_EffectUnit1]", "group_" + group + "_enable");
        } else if ((control >= 0x54) && (control <= 0x57)) {
            script.toggleControl("[EffectRack1_EffectUnit2]", "group_" + group + "_enable");
/*
        } else if ((control >= 0x70) && (control <= 0x73) && PioneerDDJSX3.shiftPanelSelectPressed) {
            script.toggleControl("[EffectRack1_EffectUnit3]", "group_" + group + "_enable");
        } else if ((control >= 0x54) && (control <= 0x57) && PioneerDDJSX3.shiftPanelSelectPressed) {
            script.toggleControl("[EffectRack1_EffectUnit4]", "group_" + group + "_enable");
*/
        }
    }
};


///////////////////////////////////////////////////////////////
//                          SLICER                           //
///////////////////////////////////////////////////////////////

PioneerDDJSX3.slicerBeatActive = function(value, group, control) {
    // This slicer implementation will work for constant beatgrids only!
    var deck = PioneerDDJSX3.channelGroups[group],
        bpm = engine.getValue(group, "bpm"),
        playposition = engine.getValue(group, "playposition"),
        duration = engine.getValue(group, "duration"),
        slicerPosInSection = 0,
        ledBeatState = true,
        domain = PioneerDDJSX3.selectedSlicerDomain[deck];

    if (engine.getValue(group, "beat_closest") === engine.getValue(group, "beat_next")) {
        return;
    }

    PioneerDDJSX3.slicerBeatsPassed[deck] = Math.round((playposition * duration) * (bpm / 60));
    slicerPosInSection = Math.floor((PioneerDDJSX3.slicerBeatsPassed[deck] % domain) / (domain / 8));

    if (PioneerDDJSX3.activePadMode[deck] === PioneerDDJSX3.padModes.slicer) {
        if (PioneerDDJSX3.activeSlicerMode[deck] === PioneerDDJSX3.slicerModes.contSlice) {
            ledBeatState = true;
        }
        if (PioneerDDJSX3.activeSlicerMode[deck] === PioneerDDJSX3.slicerModes.loopSlice) {
            ledBeatState = false;
            if (((PioneerDDJSX3.slicerBeatsPassed[deck] - 1) % domain) === (domain - 1) &&
                !PioneerDDJSX3.slicerAlreadyJumped[deck] &&
                PioneerDDJSX3.slicerPreviousBeatsPassed[deck] < PioneerDDJSX3.slicerBeatsPassed[deck]) {
                engine.setValue(group, "beatjump", -domain);
                PioneerDDJSX3.slicerAlreadyJumped[deck] = true;
            } else {
                PioneerDDJSX3.slicerAlreadyJumped[deck] = false;
            }
        }
        // PAD Led control:
        for (var i = 0; i < 8; i++) {
            if (PioneerDDJSX3.slicerActive[deck]) {
                if (PioneerDDJSX3.slicerButton[deck] !== i) {
                    PioneerDDJSX3.padLedControl(
                        group,
                        PioneerDDJSX3.ledGroups.slicer,
                        i,
                        false,
                        (slicerPosInSection === i) ? ledBeatState : !ledBeatState
                    );
                }
            } else {
                PioneerDDJSX3.padLedControl(
                    group,
                    PioneerDDJSX3.ledGroups.slicer,
                    i,
                    false,
                    (slicerPosInSection === i) ? ledBeatState : !ledBeatState
                );
            }
        }
    } else {
        PioneerDDJSX3.slicerAlreadyJumped[deck] = false;
        PioneerDDJSX3.slicerPreviousBeatsPassed[deck] = 0;
        PioneerDDJSX3.slicerActive[deck] = false;
    }
};
