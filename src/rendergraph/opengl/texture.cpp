#include "rendergraph/texture.h"

#include <qnamespace.h>
#include <qrgb.h>

#include "rendergraph/assert.h"
#include "rendergraph/context.h"

using namespace rendergraph;

namespace {
QImage premultiplyAlpha(const QImage& image) {
    // Since the image is passed by const reference, implicit copy cannot be
    // used, and this Qimage::bits will return a ref the QImage buffer, which
    // may have a shorter lifecycle that the texture buffer. In order to
    // workaround this, and because we cannot copy the image as we need to use
    // the raw bitmap with an explicit image format, we make a manual copy of
    // the buffer
    QImage result(image.width(), image.height(), QImage::Format_RGBA8888);
    if (image.format() == QImage::Format_RGBA8888_Premultiplied) {
        VERIFY_OR_DEBUG_ASSERT(result.sizeInBytes() == image.sizeInBytes()) {
            result.fill(QColor(Qt::transparent).rgba());
            return result;
        }
        std::memcpy(result.bits(), image.bits(), result.sizeInBytes());
    } else {
        auto convertedImage = image.convertToFormat(QImage::Format_RGBA8888_Premultiplied);
        VERIFY_OR_DEBUG_ASSERT(result.sizeInBytes() == convertedImage.sizeInBytes()) {
            result.fill(QColor(Qt::transparent).rgba());
            return result;
        }
        std::memcpy(result.bits(), convertedImage.bits(), result.sizeInBytes());
    }
    return result;
    /* TODO rendergraph ASK @acolombier to try if the following works as well
     * (added the .copy())
    if (image.format() == QImage::Format_RGBA8888_Premultiplied) {
        return QImage(image.bits(), image.width(), image.height(), QImage::Format_RGBA8888).copy();
    }
    return QImage(
            image.convertToFormat(QImage::Format_RGBA8888_Premultiplied)
                    .bits(),
            image.width(),
            image.height(),
            QImage::Format_RGBA8888).copy();
    */
}
} // namespace

Texture::Texture(Context* pContext, const QImage& image) {
    LLGL::TextureDescriptor texDesc;
    texDesc.type = LLGL::TextureType::Texture2D;
    texDesc.format = LLGL::Format::RGBA8UNorm;
    texDesc.extent.width = static_cast<std::uint32_t>(image.width());
    texDesc.extent.height = static_cast<std::uint32_t>(image.height());
    texDesc.extent.depth = 1;
    texDesc.bindFlags = LLGL::BindFlags::Sampled | LLGL::BindFlags::ColorAttachment;

    QImage premultiplied = premultiplyAlpha(image);

    LLGL::MipAttributedTexRegion texRegion;
    texRegion.x = 0;
    texRegion.y = 0;
    texRegion.z = 0;
    texRegion.width = static_cast<std::uint32_t>(image.width());
    texRegion.height = static_cast<std::uint32_t>(image.height());
    texRegion.depth = 1;

    // LLGL texture creation is managed via static context methods
    // For now, store the image data for later upload
    Q_UNUSED(pContext);
    Q_UNUSED(texDesc);
    Q_UNUSED(texRegion);
}

qint64 Texture::comparisonKey() const {
    return 0;
}
