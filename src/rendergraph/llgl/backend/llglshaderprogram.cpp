#include "llglshaderprogram.h"

#ifdef MIXXX_USE_LLGL

#include <QDir>
#include <QFile>
#include <QTextStream>
#include <cstring>

using namespace rendergraph;

std::uint32_t LLGLShaderProgram::s_nextShaderId = 1;

LLGLShaderProgram::LLGLShaderProgram()
        : m_pContext(nullptr),
          m_pDevice(nullptr),
          m_pCmdBuf(nullptr),
          m_pVertexShader(nullptr),
          m_pFragmentShader(nullptr),
          m_pPipelineState(nullptr),
          m_pPipelineLayout(nullptr),
          m_pVertexBuffer(nullptr),
          m_vertexStride(0),
          m_pUniformBuffer(nullptr),
          m_bound(false),
          m_shaderId(s_nextShaderId++) {
}

LLGLShaderProgram::~LLGLShaderProgram() {
    if (m_pDevice) {
        if (m_pPipelineState) m_pDevice->Release(*m_pPipelineState);
        if (m_pPipelineLayout) m_pDevice->Release(*m_pPipelineLayout);
        if (m_pVertexBuffer) m_pDevice->Release(*m_pVertexBuffer);
        if (m_pUniformBuffer) m_pDevice->Release(*m_pUniformBuffer);
        if (m_pVertexShader) m_pDevice->Release(*m_pVertexShader);
        if (m_pFragmentShader) m_pDevice->Release(*m_pFragmentShader);
    }
}

bool LLGLShaderProgram::addShaderFromSourceCode(
        const QString& vertexShader, const QString& fragmentShader) {
    if (!m_pDevice) {
        return false;
    }

    // Determine shader profiles based on backend
    const char* vsProfile = nullptr;
    const char* fsProfile = nullptr;
    const char* backendName = m_pDevice->GetName();

    if (strncmp(backendName, "OpenGL", 6) == 0) {
        vsProfile = "440";
        fsProfile = "440";
    } else if (strcmp(backendName, "Direct3D11") == 0) {
        vsProfile = "vs_5_0";
        fsProfile = "ps_5_0";
    } else if (strcmp(backendName, "Metal") == 0) {
        // Metal uses Metallib or source compilation
        vsProfile = nullptr;
        fsProfile = nullptr;
    } else if (strcmp(backendName, "Vulkan") == 0) {
        // Vulkan uses SPIR-V
        vsProfile = nullptr;
        fsProfile = nullptr;
    }

    // Create vertex shader
    LLGL::ShaderDescriptor vsDesc;
    vsDesc.type = LLGL::ShaderType::Vertex;
    QByteArray vsBytes = vertexShader.toUtf8();
    vsDesc.source = vsBytes.constData();
    vsDesc.sourceSize = static_cast<std::uint32_t>(vsBytes.size());
    vsDesc.entryPoint = "main";
    vsDesc.profile = vsProfile;

    m_pVertexShader = m_pDevice->CreateShader(vsDesc);
    if (!m_pVertexShader) {
        qWarning() << "LLGLShaderProgram: failed to create vertex shader on" << backendName;
        return false;
    }

    // Create fragment shader
    LLGL::ShaderDescriptor fsDesc;
    fsDesc.type = LLGL::ShaderType::Fragment;
    QByteArray fsBytes = fragmentShader.toUtf8();
    fsDesc.source = fsBytes.constData();
    fsDesc.sourceSize = static_cast<std::uint32_t>(fsBytes.size());
    fsDesc.entryPoint = "main";
    fsDesc.profile = fsProfile;

    m_pFragmentShader = m_pDevice->CreateShader(fsDesc);
    if (!m_pFragmentShader) {
        qWarning() << "LLGLShaderProgram: failed to create fragment shader on" << backendName;
        return false;
    }

    return true;
}

bool LLGLShaderProgram::link() {
    if (!m_pDevice || !m_pVertexShader || !m_pFragmentShader) {
        return false;
    }

    // Create pipeline layout
    LLGL::PipelineLayoutDescriptor layoutDesc;
    // Add binding points for uniform buffer at binding 0
    layoutDesc.bindingPoints.resize(1);
    layoutDesc.bindingPoints[0].type = LLGL::ResourceType::Buffer;
    layoutDesc.bindingPoints[0].bindFlags = LLGL::BindFlags::ConstantBuffer;
    layoutDesc.bindingPoints[0].stageFlags = LLGL::ShaderStageFlags::VertexStage;
    layoutDesc.bindingPoints[0].slot = 0;

    m_pPipelineLayout = m_pDevice->CreatePipelineLayout(layoutDesc);

    // Define vertex attributes (matching RGBMaterial: position + color)
    LLGL::VertexAttribute vertexAttribs[2];
    vertexAttribs[0].name = "position";
    vertexAttribs[0].format = LLGL::Format::RGBA32Float;
    vertexAttribs[0].location = 0;
    vertexAttribs[0].offset = 0;

    vertexAttribs[1].name = "color";
    vertexAttribs[1].format = LLGL::Format::RGB32Float;
    vertexAttribs[1].location = 1;
    vertexAttribs[1].offset = sizeof(float) * 4;

    // Create graphics pipeline
    LLGL::GraphicsPipelineDescriptor pipelineDesc;
    pipelineDesc.pipelineLayout = m_pPipelineLayout;
    pipelineDesc.vertexShader = m_pVertexShader;
    pipelineDesc.fragmentShader = m_pFragmentShader;
    pipelineDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleList;
    pipelineDesc.vertexAttributes = vertexAttribs;
    pipelineDesc.numVertexAttributes = 2;

    // Rasterizer: no culling, fill mode
    pipelineDesc.rasterizer.cullMode = LLGL::CullMode::Disabled;
    pipelineDesc.rasterizer.polygonMode = LLGL::PolygonMode::Fill;

    // Blend: premultiplied alpha (matching existing allshader behavior)
    pipelineDesc.blend.targets[0].blendEnabled = true;
    pipelineDesc.blend.targets[0].srcColor = LLGL::BlendOp::One;
    pipelineDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;
    pipelineDesc.blend.targets[0].colorArithmetic = LLGL::BlendArithmetic::Add;

    // Depth: disabled
    pipelineDesc.depth.testEnabled = false;
    pipelineDesc.depth.writeEnabled = false;

    m_pPipelineState = m_pDevice->CreatePipelineState(pipelineDesc);
    if (!m_pPipelineState) {
        qWarning() << "LLGLShaderProgram: failed to create pipeline state";
        return false;
    }

    // Create uniform buffer for matrix (std140 layout, 64 bytes for mat4)
    LLGL::BufferDescriptor ubDesc;
    ubDesc.size = 256; // Enough for mat4 + other uniforms
    ubDesc.bindFlags = LLGL::BindFlags::ConstantBuffer;
    ubDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;

    m_pUniformBuffer = m_pDevice->CreateBuffer(ubDesc);
    if (!m_pUniformBuffer) {
        qWarning() << "LLGLShaderProgram: failed to create uniform buffer";
        return false;
    }

    return true;
}

bool LLGLShaderProgram::bind() {
    if (!m_pCmdBuf || !m_pPipelineState) {
        return false;
    }

    m_bound = true;

    // Set pipeline state
    m_pCmdBuf->SetPipelineState(*m_pPipelineState);

    // Bind uniform buffer
    if (m_pUniformBuffer) {
        m_pCmdBuf->SetResource(0, *m_pUniformBuffer);
    }

    return true;
}

void LLGLShaderProgram::release() {
    m_bound = false;
    // LLGL doesn't need explicit unbind — pipeline state is set per draw
}

void LLGLShaderProgram::setUniformValue(int location, GLfloat value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
    // TODO: Update uniform buffer data
}

void LLGLShaderProgram::setUniformValue(int location, const QVector2D& value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShaderProgram::setUniformValue(int location, const QVector3D& value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShaderProgram::setUniformValue(int location, const QVector4D& value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShaderProgram::setUniformValue(int location, const QMatrix4x4& value) {
    Q_UNUSED(location);
    if (m_pUniformBuffer && m_pCmdBuf) {
        // Upload matrix to uniform buffer
        float matData[16];
        memcpy(matData, value.constData(), sizeof(float) * 16);
        m_pCmdBuf->UpdateBuffer(*m_pUniformBuffer, 0, matData, sizeof(matData));
    }
}

void LLGLShaderProgram::setUniformValue(int location, GLuint value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShaderProgram::enableAttributeArray(int location) {
    // LLGL vertex attributes are set via SetVertexBuffer, not individual enable/disable
    Q_UNUSED(location);
}

void LLGLShaderProgram::disableAttributeArray(int location) {
    Q_UNUSED(location);
}

void LLGLShaderProgram::setAttributeArray(int location, const float* data,
        int tupleSize, int stride) {
    Q_UNUSED(location);
    if (!m_pCmdBuf || !m_pContext) {
        return;
    }

    // Create/update vertex buffer with the geometry data
    // For now, we create a new buffer each frame (not optimal but works)
    if (m_pVertexBuffer) {
        m_pDevice->Release(*m_pVertexBuffer);
        m_pVertexBuffer = nullptr;
    }

    // Calculate total size from the data
    // Note: We need the vertex count, which we don't have here
    // This is a limitation of the current approach
    Q_UNUSED(data);
    Q_UNUSED(tupleSize);
    Q_UNUSED(stride);
}

void LLGLShaderProgram::setUniformValue(int location, QOpenGLTexture* texture) {
    Q_UNUSED(location);
    Q_UNUSED(texture);
    // TODO: Bind LLGL texture
}

int LLGLShaderProgram::uniformLocation(const char* name) const {
    Q_UNUSED(name);
    // TODO: Query from shader reflection
    return 0;
}

int LLGLShaderProgram::attributeLocation(const char* name) const {
    Q_UNUSED(name);
    // TODO: Query from shader reflection
    return -1;
}

void LLGLShaderProgram::drawArrays(GLenum mode, int first, int count) {
    Q_UNUSED(mode);
    Q_UNUSED(first);
    if (m_pCmdBuf && m_pVertexBuffer) {
        m_pCmdBuf->Draw(static_cast<std::uint32_t>(count), 0);
    }
}

#endif // MIXXX_USE_LLGL
