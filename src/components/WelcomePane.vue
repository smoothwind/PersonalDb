<template>
  <div class="welcome">
    <div class="welcome-card">
      <div class="welcome-logo">PersonalDb</div>
      <p class="welcome-sub">{{ t('welcome.subtitle') }}</p>

      <div class="welcome-steps">
        <div class="step">
          <span class="step-num">1</span>
          <span>{{ t('welcome.step1') }}</span>
        </div>
        <div class="step">
          <span class="step-num">2</span>
          <span>{{ t('welcome.step2') }}</span>
        </div>
        <div class="step">
          <span class="step-num">3</span>
          <span>{{ t('welcome.step3') }}</span>
        </div>
      </div>

      <el-button
        v-if="connections.saved.length === 0"
        type="primary"
        size="large"
        :icon="Plus"
        @click="emit('newConnection')"
      >
        {{ t('welcome.firstConnection') }}
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus } from '@element-plus/icons-vue'
import { useConnectionStore } from '../stores/connections'

defineProps<{ hasConnections: boolean }>()

const emit = defineEmits<{ (e: 'newConnection'): void }>()

const { t } = useI18n()
const connections = useConnectionStore()
</script>

<style scoped>
.welcome {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--app-welcome-bg);
}

.welcome-card {
  text-align: center;
  padding: 40px 56px;
}

.welcome-logo {
  font-size: 32px;
  font-weight: 800;
  color: var(--el-color-primary);
  letter-spacing: 1px;
}

.welcome-sub {
  margin: 8px 0 28px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

.welcome-steps {
  display: inline-flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 32px;
  text-align: left;
}

.step {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--el-text-color-regular);
  font-size: 14px;
}

.step-num {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--el-color-primary);
  color: #fff;
  font-size: 12px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
</style>
