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
          m_pUniformBuffer(nullptr),
          m_vertexStride(0),
          m_vertexCount(0),
          m_bound(false),
          m_shaderId(s_nextShaderId++) {
}

LLGLShaderProgram::~LLGLShaderProgram() {
    destroyResources();
}

void LLGLShaderProgram::destroyResources() {
    if (m_pDevice) {
        if (m_pVertexBuffer) {
            m_pDevice->Release(*m_pVertexBuffer);
            m_pVertexBuffer = nullptr;
        }
        if (m_pUniformBuffer) {
            m_pDevice->Release(*m_pUniformBuffer);
            m_pUniformBuffer = nullptr;
        }
        if (m_pPipelineState) {
            m_pDevice->Release(*m_pPipelineState);
            m_pPipelineState = nullptr;
        }
        if (m_pPipelineLayout) {
            m_pDevice->Release(*m_pPipelineLayout);
            m_pPipelineLayout = nullptr;
        }
        if (m_pVertexShader) {
            m_pDevice->Release(*m_pVertexShader);
            m_pVertexShader = nullptr;
        }
        if (m_pFragmentShader) {
            m_pDevice->Release(*m_pFragmentShader);
            m_pFragmentShader = nullptr;
        }
    }
    m_pipelineCreated = false;
}

bool LLGLShaderProgram::addShaderFromSourceCode(
        const QString& vertexShader, const QString& fragmentShader) {
    if (!m_pDevice) {
        m_vertexShaderSource = vertexShader;
        m_fragmentShaderSource = fragmentShader;
        return true;
    }

    return createPipelineState(vertexShader, fragmentShader);
}

bool LLGLShaderProgram::link() {
    if (!m_pDevice) {
        return false;
    }
    if (m_pipelineCreated) {
        return true;
    }
    return createPipelineState(m_vertexShaderSource, m_fragmentShaderSource);
}

bool LLGLShaderProgram::createPipelineState(
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
        // Need to translate GLSL to HLSL for D3D11
        qWarning() << "LLGLShaderProgram: D3D11 backend needs HLSL shaders, falling back to OpenGL";
        vsProfile = "440";
        fsProfile = "440";
    } else if (strcmp(backendName, "Metal") == 0) {
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
        qWarning() << "LLGLShaderProgram: failed to create vertex shader";
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
        qWarning() << "LLGLShaderProgram: failed to create fragment shader";
        return false;
    }

    // Pipeline layout with uniform buffer at binding 0
    LLGL::PipelineLayoutDescriptor layoutDesc;
    layoutDesc.bindingPoints.resize(1);
    layoutDesc.bindingPoints[0].type = LLGL::ResourceType::Buffer;
    layoutDesc.bindingPoints[0].bindFlags = LLGL::BindFlags::ConstantBuffer;
    layoutDesc.bindingPoints[0].stageFlags = LLGL::ShaderStageFlags::VertexStage;
    layoutDesc.bindingPoints[0].slot = 0;

    m_pPipelineLayout = m_pDevice->CreatePipelineLayout(layoutDesc);

    // Vertex attributes: position (vec4) + color (vec3)
    LLGL::VertexAttribute vertexAttribs[2];
    vertexAttribs[0].name = "position";
    vertexAttribs[0].format = LLGL::Format::RGBA32Float;
    vertexAttribs[0].location = 0;
    vertexAttribs[0].offset = 0;

    vertexAttribs[1].name = "color";
    vertexAttribs[1].format = LLGL::Format::RGB32Float;
    vertexAttribs[1].location = 1;
    vertexAttribs[1].offset = sizeof(float) * 4;

    // Graphics pipeline
    LLGL::GraphicsPipelineDescriptor pipelineDesc;
    pipelineDesc.pipelineLayout = m_pPipelineLayout;
    pipelineDesc.vertexShader = m_pVertexShader;
    pipelineDesc.fragmentShader = m_pFragmentShader;
    pipelineDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleList;
    pipelineDesc.vertexAttributes = vertexAttribs;
    pipelineDesc.numVertexAttributes = 2;

    pipelineDesc.rasterizer.cullMode = LLGL::CullMode::Disabled;
    pipelineDesc.rasterizer.polygonMode = LLGL::PolygonMode::Fill;

    pipelineDesc.blend.targets[0].blendEnabled = true;
    pipelineDesc.blend.targets[0].srcColor = LLGL::BlendOp::One;
    pipelineDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;

    pipelineDesc.depth.testEnabled = false;
    pipelineDesc.depth.writeEnabled = false;

    m_pPipelineState = m_pDevice->CreatePipelineState(pipelineDesc);
    if (!m_pPipelineState) {
        qWarning() << "LLGLShaderProgram: failed to create pipeline state on" << backendName;
        return false;
    }

    // Create uniform buffer (256 bytes for mat4 + other uniforms)
    LLGL::BufferDescriptor ubDesc;
    ubDesc.size = 256;
    ubDesc.bindFlags = LLGL::BindFlags::ConstantBuffer;
    ubDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;

    m_pUniformBuffer = m_pDevice->CreateBuffer(ubDesc);
    if (!m_pUniformBuffer) {
        qWarning() << "LLGLShaderProgram: failed to create uniform buffer";
        return false;
    }

    // Cache attribute locations
    m_attributeLocations.clear();
    m_attributeLocations.push_back(0); // position at location 0
    m_attributeLocations.push_back(1); // color at location 1

    // Cache uniform locations
    m_uniformLocations.clear();
    m_uniformLocations.push_back(0); // matrix at location 0

    m_pipelineCreated = true;
    qDebug() << "LLGLShaderProgram: shaders loaded on" << backendName;
    return true;
}

bool LLGLShaderProgram::bind() {
    if (!m_pCmdBuf || !m_pPipelineState) {
        return false;
    }

    m_bound = true;

    // Set pipeline state
    m_pCmdBuf->SetPipelineState(*m_pPipelineState);

    // Bind uniform buffer at slot 0
    if (m_pUniformBuffer) {
        m_pCmdBuf->SetResource(0, *m_pUniformBuffer);
    }

    return true;
}

void LLGLShaderProgram::release() {
    m_bound = false;
}

void LLGLShaderProgram::setUniformValue(int location, GLfloat value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
    // TODO: update uniform buffer for float uniforms
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
    if (m_pUniformBuffer && m_pCmdBuf && m_bound) {
        // Upload matrix to uniform buffer (offset 0, 64 bytes for mat4)
        const float* data = value.constData();
        m_pCmdBuf->UpdateBuffer(*m_pUniformBuffer, 0, data, 64);
    }
}

void LLGLShaderProgram::setUniformValue(int location, GLuint value) {
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShaderProgram::enableAttributeArray(int location) {
    Q_UNUSED(location);
    // LLGL vertex attributes are set via SetVertexBuffer
}

void LLGLShaderProgram::disableAttributeArray(int location) {
    Q_UNUSED(location);
}

void LLGLShaderProgram::setAttributeArray(int location, const float* data,
        int tupleSize, int stride) {
    Q_UNUSED(location);
    Q_UNUSED(data);
    Q_UNUSED(tupleSize);
    Q_UNUSED(stride);
    // Vertex buffer is created in drawArrays from the geometry data
}

void LLGLShaderProgram::setUniformValue(int location, QOpenGLTexture* texture) {
    Q_UNUSED(location);
    Q_UNUSED(texture);
    // Texture binding not yet implemented
}

int LLGLShaderProgram::uniformLocation(const char* name) const {
    Q_UNUSED(name);
    return 0;
}

int LLGLShaderProgram::attributeLocation(const char* name) const {
    if (strcmp(name, "position") == 0) return 0;
    if (strcmp(name, "color") == 0) return 1;
    return -1;
}

void LLGLShaderProgram::drawArrays(GLenum mode, int first, int count) {
    Q_UNUSED(mode);
    Q_UNUSED(first);
    if (!m_pCmdBuf || !m_pVertexBuffer || count == 0) {
        return;
    }

    // Set vertex buffer
    m_pCmdBuf->SetVertexBuffer(*m_pVertexBuffer);

    // Draw
    m_pCmdBuf->Draw(static_cast<std::uint32_t>(count), static_cast<std::uint32_t>(first));
}

void LLGLShaderProgram::updateVertexBuffer(const float* data, std::uint32_t vertexCount, std::uint32_t stride) {
    if (!m_pDevice || !m_pCmdBuf || data == nullptr || vertexCount == 0) {
        return;
    }

    std::uint32_t dataSize = vertexCount * stride;

    // Recreate buffer if size changed
    if (m_pVertexBuffer && m_vertexCount != vertexCount) {
        m_pDevice->Release(*m_pVertexBuffer);
        m_pVertexBuffer = nullptr;
    }

    if (!m_pVertexBuffer) {
        LLGL::BufferDescriptor vbDesc;
        vbDesc.size = dataSize;
        vbDesc.bindFlags = LLGL::BindFlags::VertexBuffer;
        vbDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;

        m_pVertexBuffer = m_pDevice->CreateBuffer(vbDesc, data);
        m_vertexCount = vertexCount;
        m_vertexStride = stride;
    } else {
        // Update existing buffer
        m_pCmdBuf->UpdateBuffer(*m_pVertexBuffer, 0, data, dataSize);
    }
}

#endif // MIXXX_USE_LLGL
