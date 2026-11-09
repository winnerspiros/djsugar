# R8 removes the unused library code (androidx, kotlinx, kotlin). Classes
# reached from C++ through JNI, by name or by reflection are kept whole.
-dontobfuscate

# Qt: called from its C++ side through JNI
-keep class org.qtproject.qt.android.** { *; }
# Mixxx: activity, services and the helpers called from C++
-keep class org.mixxx.** { *; }
# yt-dlp / aria2c wrappers: called from C++, results read field by field
-keep class com.yausername.** { *; }
# Jackson maps the yt-dlp JSON onto the classes above by reflection
-keep class com.fasterxml.jackson.** { *; }
# commons-compress unzips the bundled Python/yt-dlp/aria2c; ExtraFieldUtils
# instantiates the zip extra-field classes by reflection. Shrunk, they turn
# abstract ("AsiExtraField is not a concrete class") and YoutubeDL.init fails.
-keep class org.apache.commons.compress.** { *; }
-keep class org.apache.commons.io.** { *; }
-keepattributes *Annotation*,Signature,InnerClasses,EnclosingMethod

-keepclasseswithmembernames class * {
    native <methods>;
}

-dontwarn **
