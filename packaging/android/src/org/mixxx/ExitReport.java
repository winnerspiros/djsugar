package org.mixxx;

import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.Context;
import android.os.Build;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.List;

/// How the app's previous run ended, from Android's own record: it knows of
/// crashes the app could not write down (a native crash on a thread without
/// a usable stack, the system killing it), for the log
public class ExitReport {
    private static final int MAX_TRACE_BYTES = 4 * 1024 * 1024;
    private static final int MAX_REPORT_CHARS = 256 * 1024;

    /// "" when Android has no record (before Android 11) or the run ended
    /// normally
    public static String previousExit(Context context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            return "";
        }
        ActivityManager manager =
            (ActivityManager) context.getSystemService(Context.ACTIVITY_SERVICE);
        List<ApplicationExitInfo> exits =
            manager.getHistoricalProcessExitReasons(context.getPackageName(), 0, 1);
        if (exits.isEmpty()) {
            return "";
        }
        ApplicationExitInfo exit = exits.get(0);
        int reason = exit.getReason();
        if (reason == ApplicationExitInfo.REASON_EXIT_SELF
            || reason == ApplicationExitInfo.REASON_USER_REQUESTED) {
            return "";
        }
        StringBuilder report = new StringBuilder();
        report.append("reason ").append(reason).append(" (4 crash, 5 native crash, 6 not responding, 3 low memory)").append(", status ").append(exit.getStatus()).append(", at ").append(new java.util.Date(exit.getTimestamp())).append(", memory ").append(exit.getPss() / 1024).append(" MiB").append(", ").append(exit.getDescription());
        String trace = readableTrace(exit);
        if (!trace.isEmpty()) {
            report.append("\n").append(trace);
        }
        return report.length() > MAX_REPORT_CHARS
            ? report.substring(0, MAX_REPORT_CHARS)
            : report.toString();
    }

    /// The trace of a crash or hang: text for a Java crash or hang, a binary
    /// tombstone for a native crash, of which the readable runs (thread
    /// names, signal, library and function names) are kept
    private static String readableTrace(ApplicationExitInfo exit) {
        try (InputStream in = exit.getTraceInputStream()) {
            if (in == null) {
                return "";
            }
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            byte[] buffer = new byte[16384];
            int n;
            while ((n = in.read(buffer)) > 0 && bytes.size() < MAX_TRACE_BYTES) {
                bytes.write(buffer, 0, n);
            }
            StringBuilder out = new StringBuilder();
            StringBuilder run = new StringBuilder();
            for (byte b : bytes.toByteArray()) {
                if (b >= 0x20 && b < 0x7f || b == '\n') {
                    run.append((char) b);
                    continue;
                }
                if (run.length() >= 4) {
                    out.append(run).append('\n');
                }
                run.setLength(0);
            }
            if (run.length() >= 4) {
                out.append(run);
            }
            return out.toString();
        } catch (Exception e) {
            return "trace: " + e;
        }
    }
}
