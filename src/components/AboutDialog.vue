<template>
  <el-dialog
    v-model="visible"
    width="420px"
    align-center
    class="about-dialog"
    :show-close="false"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_info')" /></el-icon>
        {{ t('menu.about') }}
      </span>
    </template>
    <div class="about">
      <div class="about-logo">PersonalDb</div>
      <div class="about-sub">{{ t('menu.aboutBody') }}</div>
      <el-tag size="small" effect="plain" class="about-ver">v0.1.0</el-tag>
      <div class="about-stack">
        <el-tag size="small" effect="plain">Tauri 2</el-tag>
        <el-tag size="small" effect="plain">WebView2</el-tag>
        <el-tag size="small" effect="plain">Vue 3</el-tag>
      </div>
      <div class="about-credits">{{ t('menu.aboutCredits') }}</div>
    </div>
    <template #footer>
      <el-button size="small" type="primary" @click="visible = false">
        {{ t('common.confirm') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'

const { t } = useI18n()

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v),
})
</script>

<style scoped>
.about {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.about-logo {
  font-size: 26px;
  font-weight: 700;
  background: linear-gradient(135deg, var(--el-color-primary), var(--el-color-primary-light-3));
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.about-sub {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.about-ver {
  margin-top: 2px;
}
.about-stack {
  display: flex;
  gap: 6px;
}
.about-credits {
  font-size: 11px;
  opacity: 0.55;
  margin-top: 4px;
}
</style>
