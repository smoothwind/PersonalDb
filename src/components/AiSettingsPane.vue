<template>
  <div class="ai-cfg">
    <!-- 提供方列表 -->
    <div class="ai-cfg-row">
      <el-button size="small" type="primary" plain @click="openAdd">
        {{ t('aiCfg.addProvider') }}
      </el-button>
      <el-button
        v-if="aiStore.active"
        size="small"
        :loading="testing"
        @click="testActive"
      >
        {{ t('aiCfg.test') }}
      </el-button>
      <span v-if="!aiStore.prefs.providers.length" class="ai-cfg-empty">{{ t('aiCfg.providersEmpty') }}</span>
    </div>

    <div v-for="p in aiStore.prefs.providers" :key="p.id" class="ai-cfg-item" :class="{ 'is-active': p.id === aiStore.prefs.activeId }">
      <el-radio
        :model-value="aiStore.prefs.activeId"
        :value="p.id"
        size="small"
        @update:model-value="aiStore.setActive(String(p.id))"
      >
        {{ p.name }}
      </el-radio>
      <span class="ai-cfg-meta">{{ p.model }}</span>
      <span class="ai-cfg-meta ai-cfg-url">{{ p.baseURL }}</span>
      <span class="ai-cfg-key">{{ p.passwordRef ? t('aiCfg.apiKeySet') : t('aiCfg.anonymous') }}</span>
      <span class="flex-sp" />
      <el-button size="small" text @click="openEdit(p)">{{ t('common.edit') }}</el-button>
      <el-button size="small" text type="danger" @click="confirmRemove(p)">{{ t('common.delete') }}</el-button>
    </div>

    <!-- 隐私开关 -->
    <div class="ai-cfg-section">{{ t('aiCfg.privacy') }}</div>
    <el-form label-width="150px" size="small">
      <el-form-item :label="t('ai.contextEnabled')">
        <el-switch
          :model-value="aiStore.prefs.contextEnabled"
          @update:model-value="(v: boolean | string | number) => aiStore.updateFlags({ contextEnabled: !!v })"
        />
        <span class="ai-cfg-hint">{{ t('aiCfg.contextHint') }}</span>
      </el-form-item>
      <el-form-item :label="t('ai.insightTitle')">
        <el-switch
          :model-value="aiStore.prefs.insightEnabled"
          @update:model-value="(v: boolean | string | number) => aiStore.updateFlags({ insightEnabled: !!v })"
        />
        <span class="ai-cfg-hint">{{ t('aiCfg.insightHint') }}</span>
      </el-form-item>
    </el-form>

    <!-- 提供方编辑对话框 -->
    <el-dialog
      v-model="dlgVisible"
      :title="editing ? t('aiCfg.editProvider') : t('aiCfg.addProvider')"
      width="520px"
      append-to-body
    >
      <el-form label-width="130px" size="small">
        <el-form-item :label="t('aiCfg.preset')">
          <el-select v-model="presetKey" :placeholder="t('aiCfg.presetCustom')" clearable @change="applyPreset">
            <el-option v-for="ps in AI_PRESETS" :key="ps.key" :value="ps.key" :label="t(ps.labelKey)" />
          </el-select>
        </el-form-item>
        <el-form-item :label="t('aiCfg.name')" required>
          <el-input v-model="form.name" maxlength="30" />
        </el-form-item>
        <el-form-item :label="t('aiCfg.baseURL')" required>
          <el-input v-model="form.baseURL" placeholder="https://api.deepseek.com/v1" />
        </el-form-item>
        <el-form-item :label="t('aiCfg.model')" required>
          <el-input v-model="form.model" placeholder="deepseek-chat" />
        </el-form-item>
        <el-form-item :label="t('aiCfg.apiKey')">
          <el-input
            v-model="apiKeyInput"
            type="password"
            show-password
            :placeholder="form.passwordRef ? t('aiCfg.apiKeySet') : t('aiCfg.apiKeyNone')"
          />
        </el-form-item>
        <el-collapse class="ai-cfg-adv">
          <el-collapse-item :title="t('aiCfg.advanced')">
            <el-form-item :label="t('aiCfg.temperature')">
              <el-input-number v-model="form.temperature" :min="0" :max="2" :step="0.1" />
            </el-form-item>
            <el-form-item :label="t('aiCfg.maxTokens')">
              <el-input-number v-model="form.maxTokens" :min="16" :max="65536" :step="256" />
            </el-form-item>
            <el-form-item :label="t('aiCfg.timeout')">
              <el-input-number v-model="form.timeoutSecs" :min="10" :max="600" :step="10" />
            </el-form-item>
          </el-collapse-item>
        </el-collapse>
      </el-form>
      <template #footer>
        <el-button size="small" @click="dlgVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button size="small" :loading="testingInDlg" @click="testForm">{{ t('aiCfg.test') }}</el-button>
        <el-button size="small" type="primary" @click="save">{{ t('common.save') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { AI_PRESETS, providerFromPreset } from '../utils/aiPresets'
import { useAiStore } from '../stores/ai'
import type { AiProviderCfg } from '../types/ai'

const { t } = useI18n()
const aiStore = useAiStore()

const dlgVisible = ref(false)
const editing = ref<AiProviderCfg | null>(null)
const presetKey = ref<string>('')
const apiKeyInput = ref('')
const testing = ref(false)
const testingInDlg = ref(false)

const form = reactive<AiProviderCfg>({
  id: '',
  name: '',
  baseURL: '',
  model: '',
  passwordRef: null,
  temperature: null,
  maxTokens: null,
  timeoutSecs: null,
})

function resetForm(): void {
  form.id = ''
  form.name = ''
  form.baseURL = ''
  form.model = ''
  form.passwordRef = null
  form.temperature = null
  form.maxTokens = null
  form.timeoutSecs = null
  apiKeyInput.value = ''
  presetKey.value = ''
}

function openAdd(): void {
  resetForm()
  editing.value = null
  dlgVisible.value = true
}

function openEdit(p: AiProviderCfg): void {
  resetForm()
  Object.assign(form, p)
  apiKeyInput.value = ''
  editing.value = p
  dlgVisible.value = true
}

function applyPreset(key: string): void {
  const ps = AI_PRESETS.find((x) => x.key === key)
  if (!ps) return
  form.baseURL = ps.baseURL
  form.model = ps.defaultModel
  if (!form.name) form.name = ps.key
}

async function save(): Promise<void> {
  if (!form.name.trim() || !form.baseURL.trim() || !form.model.trim()) {
    ElMessage.warning(t('aiCfg.requiredMissing'))
    return
  }
  const p: AiProviderCfg = { ...form, name: form.name.trim() }
  if (editing.value) {
    p.id = editing.value.id
    // 编辑保留原 keyring 引用（除非用户清空/重填 key）
    if (apiKeyInput.value === '' && editing.value.passwordRef) p.passwordRef = editing.value.passwordRef
  } else {
    const preset = AI_PRESETS.find((x) => x.key === presetKey.value)
    const base = preset ? providerFromPreset(preset) : { ...p, id: `ai-custom-${Date.now().toString(36)}`, passwordRef: `ai:${p.id || 'x'}` }
    p.id = base.id
    if (!apiKeyInput.value && preset?.anonymous) p.passwordRef = null
    else p.passwordRef = `ai:${p.id}`
  }
  await aiStore.upsertProvider(p, apiKeyInput.value === '' ? undefined : apiKeyInput.value)
  dlgVisible.value = false
  ElMessage.success(t('common.saved'))
}

async function testForm(): Promise<void> {
  if (!form.baseURL.trim() || !form.model.trim()) {
    ElMessage.warning(t('aiCfg.requiredMissing'))
    return
  }
  testingInDlg.value = true
  try {
    // 表单探测：临时把 key 直接带过去（走内存构造，不经 keyring）
    const draft: AiProviderCfg = { ...form, passwordRef: apiKeyInput.value ? `ai:${form.id || 'draft'}` : form.passwordRef }
    if (apiKeyInput.value) await aiStore.upsertProvider({ ...draft, id: draft.id || 'draft' }, apiKeyInput.value)
    const reply = await aiStore.testConnection(draft)
    ElMessage.success(t('aiCfg.testOk', { reply: reply.slice(0, 60) }))
  } catch (e) {
    ElMessage.error(e instanceof Error ? e.message : String(e))
  } finally {
    testingInDlg.value = false
  }
}

async function testActive(): Promise<void> {
  const p = aiStore.active
  if (!p) return
  testing.value = true
  try {
    const reply = await aiStore.testConnection(p)
    ElMessage.success(t('aiCfg.testOk', { reply: reply.slice(0, 60) }))
  } catch (e) {
    ElMessage.error(e instanceof Error ? e.message : String(e))
  } finally {
    testing.value = false
  }
}

async function confirmRemove(p: AiProviderCfg): Promise<void> {
  await ElMessageBox.confirm(t('aiCfg.confirmDelete', { name: p.name }), t('common.delete'), {
    type: 'warning',
  })
  await aiStore.removeProvider(p.id)
}
</script>

<style scoped>
.ai-cfg {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ai-cfg-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ai-cfg-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.ai-cfg-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 8px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}

.ai-cfg-item.is-active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

.ai-cfg-meta {
  font-size: 12px;
  color: var(--el-text-color-regular);
}

.ai-cfg-url {
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 260px;
}

.ai-cfg-key {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.ai-cfg-section {
  margin-top: 8px;
  font-weight: 600;
  font-size: 13px;
}

.ai-cfg-hint {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.ai-cfg-adv {
  border: none;
}

.flex-sp {
  flex: 1;
}
</style>
