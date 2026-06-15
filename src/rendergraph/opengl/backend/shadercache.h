#pragma once

#include <unordered_map>
#include <memory>

#include "rendergraph/material.h"
#include "llglmaterialshader.h"

namespace rendergraph {

class ShaderCache {
  private:
    static std::unordered_map<MaterialType*,
            std::shared_ptr<LLGLMaterialShader>>&
    map() {
        static std::unordered_map<MaterialType*,
                std::shared_ptr<LLGLMaterialShader>>
                s_map;
        return s_map;
    }

  public:
    static std::shared_ptr<LLGLMaterialShader> getShaderForMaterial(
            Material* pMaterial) {
        auto iter = map().find(pMaterial->type());
        if (iter != map().end()) {
            return iter->second;
        }
        auto pResult = std::shared_ptr<LLGLMaterialShader>(
                pMaterial->createLLGLShader());
        map().insert(std::pair<MaterialType*,
                std::shared_ptr<LLGLMaterialShader>>{
                pMaterial->type(), pResult});
        return pResult;
    }
    static void purge() {
        std::erase_if(map(), [](const auto& item) {
            auto const& [key, value] = item;
            return value.use_count() == 1;
        });
    }
};

} // namespace rendergraph
