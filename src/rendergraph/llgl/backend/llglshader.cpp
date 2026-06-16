#include "llglshader.h"

#ifdef MIXXX_USE_LLGL

#include <QDebug>
#include <cstring>

using namespace rendergraph;

bool LLGLShader::load(LLGL::RenderSystem* pDevice,
        const QString& vertexShaderCode,
        const QString& fragmentShaderCode,
        const LLGL::VertexAttribute* pAttribs,
        std::uint32_t numAttribs) {
    if (!pDevice) {
        return false;
    }

    // Determine shader version/profile based on backend
    const char* vsProfile = nullptr;
    const char* fsProfile = nullptr;

    const char* backendName = pDevice->GetName();
    if (strcmp(backendName, "OpenGL") == 0 || strcmp(backendName, "OpenGLES") == 0) {
        vsProfile = "330 core";
        fsProfile = "330 core";
    } else if (strcmp(backendName, "Direct3D11") == 0) {
        vsProfile = "vs_5_0";
        fsProfile = "ps_5_0";
    } else if (strcmp(backendName, "Metal") == 0) {
        vsProfile = "2.0";
        fsProfile = "2.0";
    }

    // Create vertex shader
    LLGL::ShaderDescriptor vsDesc;
    vsDesc.type = LLGL::ShaderType::Vertex;
    vsDesc.source = vertexShaderCode.toUtf8().constData();
    vsDesc.sourceSize = static_cast<std::uint32_t>(vertexShaderCode.size());
    vsDesc.entryPoint = "main";
    vsDesc.profile = vsProfile;

    m_pVertexShader = pDevice->CreateShader(vsDesc);
    if (!m_pVertexShader) {
        qWarning() << "LLGLShader: failed to create vertex shader";
        return false;
    }

    // Create fragment shader
    LLGL::ShaderDescriptor fsDesc;
    fsDesc.type = LLGL::ShaderType::Fragment;
    fsDesc.source = fragmentShaderCode.toUtf8().constData();
    fsDesc.sourceSize = static_cast<std::uint32_t>(fragmentShaderCode.size());
    fsDesc.entryPoint = "main";
    fsDesc.profile = fsProfile;

    m_pFragmentShader = pDevice->CreateShader(fsDesc);
    if (!m_pFragmentShader) {
        qWarning() << "LLGLShader: failed to create fragment shader";
        return false;
    }

    // Pipeline layout (empty for now)
    LLGL::PipelineLayoutDescriptor layoutDesc;
    m_pPipelineLayout = pDevice->CreatePipelineLayout(layoutDesc);

    // Graphics pipeline
    LLGL::GraphicsPipelineDescriptor pipelineDesc;
    pipelineDesc.pipelineLayout = m_pPipelineLayout;
    pipelineDesc.vertexShader = m_pVertexShader;
    pipelineDesc.fragmentShader = m_pFragmentShader;
    pipelineDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleList;

    if (pAttribs && numAttribs > 0) {
        pipelineDesc.vertexAttributes = pAttribs;
        pipelineDesc.numVertexAttributes = numAttribs;
    }

    // Rasterizer
    pipelineDesc.rasterizer.cullMode = LLGL::CullMode::Disabled;
    pipelineDesc.rasterizer.polygonMode = LLGL::PolygonMode::Fill;

    // Blend
    pipelineDesc.blend.targets[0].blendEnabled = true;
    pipelineDesc.blend.targets[0].srcColor = LLGL::BlendOp::SrcAlpha;
    pipelineDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;

    // Depth
    pipelineDesc.depth.testEnabled = false;
    pipelineDesc.depth.writeEnabled = false;

    m_pPipelineState = pDevice->CreatePipelineState(pipelineDesc);
    if (!m_pPipelineState) {
        qWarning() << "LLGLShader: failed to create pipeline state";
        return false;
    }

    // Cache attribute locations
    m_attributeLocations.clear();
    for (std::uint32_t i = 0; i < numAttribs; i++) {
        m_attributeLocations.push_back(static_cast<int>(pAttribs[i].location));
    }

    // Cache uniform locations (query from shader reflection)
    m_uniformLocations.clear();
    if (m_pVertexShader) {
        auto* pReflection = pDevice->GetShaderReflection(*m_pVertexShader);
        if (pReflection) {
            for (std::uint32_t i = 0; i < pReflection->uniforms.numUniforms; ++i) {
                m_uniformLocations.push_back(static_cast<int>(i));
            }
        }
    }

    qDebug() << "LLGLShader: loaded successfully on" << backendName;
    return true;
}

void LLGLShader::setUniform(LLGL::CommandBuffer* pCmdBuf,
        int location,
        float value) {
    Q_UNUSED(pCmdBuf);
    Q_UNUSED(location);
    Q_UNUSED(value);
    // TODO: Implement via push constants or uniform buffer
}

void LLGLShader::setUniform(LLGL::CommandBuffer* pCmdBuf,
        int location,
        const QVector2D& value) {
    Q_UNUSED(pCmdBuf);
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShader::setUniform(LLGL::CommandBuffer* pCmdBuf,
        int location,
        const QVector3D& value) {
    Q_UNUSED(pCmdBuf);
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShader::setUniform(LLGL::CommandBuffer* pCmdBuf,
        int location,
        const QVector4D& value) {
    Q_UNUSED(pCmdBuf);
    Q_UNUSED(location);
    Q_UNUSED(value);
}

void LLGLShader::setUniform(LLGL::CommandBuffer* pCmdBuf,
        int location,
        const QMatrix4x4& value) {
    Q_UNUSED(pCmdBuf);
    Q_UNUSED(location);
    Q_UNUSED(value);
}

#endif // MIXXX_USE_LLGL
