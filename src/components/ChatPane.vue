<template>
  <div class="chat-pane">
    <!-- 头部：提供方状态 + 清空会话 -->
    <div class="chat-head">
      <el-icon class="chat-logo"><component :is="objectIcon('act_ai')" /></el-icon>
      <span class="chat-title">{{ t('ai.chatTitle') }}</span>
      <span v-if="activeName" class="chat-provider">{{ activeName }}</span>
      <span class="flex-sp" />
      <el-tooltip :content="t('ai.settings')" placement="bottom">
        <button class="chat-btn" type="button" @click="openSettings">
          <el-icon><component :is="objectIcon('act_settings')" /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip :content="t('ai.clearChat')" placement="bottom">
        <button class="chat-btn" type="button" @click="confirmClear">
          <el-icon><component :is="objectIcon('act_delete')" /></el-icon>
        </button>
      </el-tooltip>
    </div>

    <!-- 消息列表 -->
    <div ref="listRef" class="chat-list">
      <div v-if="!aiStore.chatMsgs.length && !aiStore.running" class="chat-empty">
        {{ t('ai.chatEmpty') }}
      </div>
      <div
        v-for="(m, i) in aiStore.chatMsgs"
        :key="i"
        class="chat-msg"
        :class="m.role === 'user' ? 'is-user' : m.role === 'assistant' ? 'is-assistant' : 'is-system'"
      >
        <pre class="chat-text">{{ m.content }}</pre>
      </div>
      <div v-if="aiStore.running" class="chat-msg is-assistant">
        <pre class="chat-text">{{ streamingText }}<span class="chat-cursor">▌</span></pre>
      </div>
    </div>

    <!-- 错误条 -->
    <div v-if="errorMsg" class="chat-error">{{ errorMsg }}</div>

    <!-- 输入区 -->
    <div class="chat-input-row">
      <el-input
        v-model="input"
        type="textarea"
        :rows="3"
        resize="none"
        :placeholder="t('ai.phChat')"
        @keydown.enter.exact.prevent="send"
        @keydown.ctrl.enter.prevent="send"
        @keydown.meta.enter.prevent="send"
      />
      <div class="chat-actions">
        <el-button type="primary" size="small" :disabled="!canSend" @click="send">
          {{ t('ai.send') }}
        </el-button>
        <el-button v-if="aiStore.running" size="small" type="warning" plain @click="aiStore.cancel()">
          {{ t('ai.stop') }}
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { dispatchCommand } from '../utils/commands'
import { objectIcon } from '../utils/objectIcons'
import { buildChat } from '../utils/aiPrompt'
import { logger } from '../utils/logger'
import { useAiStore } from '../stores/ai'
import type { QueryTab } from '../stores/tabs'

const props = defineProps<{ tab: QueryTab }>()

const { t } = useI18n()
const aiStore = useAiStore()

const input = ref('')
const errorMsg = ref('')
const streamingText = ref('')
const listRef = ref<HTMLElement>()

const activeName = computed(() => aiStore.active?.name ?? '')
const canSend = computed(
  () => !aiStore.running && !!aiStore.active && input.value.trim().length > 0,
)

function scrollBottom(): void {
  void nextTick(() => {
    const el = listRef.value
    if (el) el.scrollTop = el.scrollHeight
  })
}

function send(): void {
  if (!canSend.value) return
  const question = input.value.trim()
  input.value = ''
  errorMsg.value = ''
  streamingText.value = ''
  aiStore.pushChat({ role: 'user', content: question })
  scrollBottom()
  // 多轮上下文：持久化历史（不含 system 提示）+ 方言提示
  const msgs = buildChat(props.tab.kind, aiStore.chatMsgs.slice(0, -1), question)
  void aiStore
    .stream(
      msgs,
      (delta) => {
        streamingText.value += delta
        scrollBottom()
      },
      () => {
        if (streamingText.value) aiStore.pushChat({ role: 'assistant', content: streamingText.value })
        streamingText.value = ''
        scrollBottom()
      },
      (msg) => {
        errorMsg.value = msg
        logger.warn(`AI 对话流失败: ${msg}`)
        streamingText.value = ''
      },
    )
    .catch((e: unknown) => {
      errorMsg.value = e instanceof Error ? e.message : String(e)
      logger.warn(`AI 对话调用异常: ${errorMsg.value}`)
    })
}

function openSettings(): void {
  dispatchCommand('open-settings', undefined, 'ai')
}

async function confirmClear(): Promise<void> {
  await ElMessageBox.confirm(t('ai.clearChatConfirm'), t('ai.clearChat'), { type: 'warning' })
  aiStore.clearChat()
  ElMessage.success(t('ai.chatCleared'))
}
</script>

<style scoped>
.chat-pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.chat-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
}

.chat-logo {
  color: var(--el-color-primary);
}

.chat-title {
  font-weight: 600;
  font-size: 13px;
}

.chat-provider {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.flex-sp {
  flex: 1;
}

.chat-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--el-text-color-regular);
  cursor: pointer;
  border-radius: 4px;
}

.chat-btn:hover {
  background: var(--el-fill-color);
}

.chat-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
}

.chat-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chat-empty {
  margin-top: 60px;
  text-align: center;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.chat-msg {
  max-width: 78%;
  border-radius: 8px;
  padding: 6px 10px;
}

.chat-msg.is-user {
  align-self: flex-end;
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary-light-7);
}

.chat-msg.is-assistant {
  align-self: flex-start;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
}

.chat-msg.is-system {
  display: none;
}

.chat-text {
  margin: 0;
  font-family: inherit;
  font-size: 12.5px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.chat-cursor {
  animation: chat-blink 1s step-start infinite;
  color: var(--el-color-primary);
}

@keyframes chat-blink {
  50% {
    opacity: 0;
  }
}

.chat-error {
  padding: 4px 16px;
  font-size: 12px;
  color: var(--el-color-danger);
  flex-shrink: 0;
}

.chat-input-row {
  padding: 8px 12px;
  border-top: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
}

.chat-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 6px;
}
</style>
