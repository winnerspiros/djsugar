#pragma once

#include <QtGlobal>

// This define is checked in wglwidgetqglwidget.h and wglwidgetqglwidget.h
// to make sure they are only included from this header, to enforce that
// all code includes this header wglwidget.h.
#define WGLWIDGET_H

#ifdef Q_OS_ANDROID
// Android: use stub to avoid QOpenGLWindow crash
#include "widget/wglwidgetqopengl_android.h"
#elif MIXXX_USE_QOPENGL
#include "widget/wglwidgetqopengl.h"
#elif QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
// QGLWidget was removed in Qt6 — only include Qt5 fallback
#include "widget/wglwidgetqglwidget.h"
#else
// Qt6 without QOPENGL: use QOpenGLWindow-based fallback
#include "widget/wglwidgetqopengl.h"
#endif

#undef WGLWIDGET_H
