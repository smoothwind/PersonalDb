<template>
  <el-dialog
    :model-value="modelValue"
    width="720px"
    top="8vh"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_settings')" /></el-icon>
        {{ t('settings.title') }}
      </span>
    </template>
    <div class="settings-layout">
      <!-- 左侧分类导航 -->
      <aside class="settings-nav">
        <div
          v-for="cat in categories"
          :key="cat.key"
          class="nav-item"
          :class="{ 'is-active': active === cat.key }"
          @click="active = cat.key"
        >
          <el-icon><component :is="cat.icon" /></el-icon>
          <span>{{ cat.label }}</span>
        </div>
      </aside>

      <!-- 右侧内容 -->
      <main class="settings-content">
        <!-- 外观 -->
        <template v-if="active === 'appearance'">
          <div class="pref-section">{{ t('settings.appearance') }}</div>
          <el-form label-width="120px" size="small">
            <el-form-item :label="t('settings.theme')">
              <el-radio-group :model-value="prefs.theme" @update:model-value="prefs.setTheme">
                <el-radio-button value="light">{{ t('settings.themeLight') }}</el-radio-button>
                <el-radio-button value="dark">{{ t('settings.themeDark') }}</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.language')">
              <el-radio-group :model-value="prefs.locale" @update:model-value="prefs.changeLocale">
                <el-radio-button value="zh-CN">{{ t('settings.languageZh') }}</el-radio-button>
                <el-radio-button value="en">{{ t('settings.languageEn') }}</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.uiFontFamily')">
              <el-input
                :model-value="prefs.ui.fontFamily"
                :placeholder="t('settings.uiFontFamilyPlaceholder')"
                @update:model-value="(v: string) => prefs.updateUi({ fontFamily: v })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.uiFontSize')">
              <el-input-number
                :model-value="prefs.ui.fontSize"
                :min="10"
                :max="24"
                controls-position="right"
                @update:model-value="(v: number | undefined) => prefs.updateUi({ fontSize: v ?? 14 })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.uiLineHeight')">
              <el-input-number
                :model-value="prefs.ui.lineHeight"
                :min="1"
                :max="3"
                :step="0.1"
                :precision="1"
                controls-position="right"
                @update:model-value="(v: number | undefined) => prefs.updateUi({ lineHeight: v ?? 1.5 })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.maxTabs')">
              <el-input-number
                :model-value="prefs.ui.maxTabs"
                :min="1"
                :max="200"
                controls-position="right"
                @update:model-value="(v: number | undefined) => prefs.updateUi({ maxTabs: Math.min(200, Math.max(1, v ?? 50)) })"
              />
            </el-form-item>
          </el-form>
        </template>

        <!-- 编辑器 -->
        <template v-else-if="active === 'editor'">
          <div class="pref-section">{{ t('settings.editorSection') }}</div>
          <el-form label-width="120px" size="small">
            <el-form-item :label="t('settings.editorFontSize')">
              <el-input-number
                :model-value="prefs.editor.fontSize"
                :min="11"
                :max="22"
                controls-position="right"
                style="width: 130px"
                @update:model-value="(v: number | undefined) => prefs.updateEditor({ fontSize: v ?? 14 })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.editorTabSize')">
              <el-input-number
                :model-value="prefs.editor.tabSize"
                :min="1"
                :max="8"
                controls-position="right"
                style="width: 130px"
                @update:model-value="(v: number | undefined) => prefs.updateEditor({ tabSize: v ?? 2 })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.editorLineNumbers')">
              <el-switch
                :model-value="prefs.editor.lineNumbers"
                @update:model-value="(v: string | number | boolean) => prefs.updateEditor({ lineNumbers: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.editorWrap')">
              <el-switch
                :model-value="prefs.editor.wrap"
                @update:model-value="(v: string | number | boolean) => prefs.updateEditor({ wrap: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.editorKeywordUpper')">
              <el-switch
                :model-value="prefs.editor.upperCaseKeywords"
                @update:model-value="(v: string | number | boolean) => prefs.updateEditor({ upperCaseKeywords: Boolean(v) })"
              />
            </el-form-item>

            <div class="pref-section">
              {{ t('settings.formatSection') }}
              <span class="preset-actions">
                <el-button size="small" @click="prefs.updateFormat(plsqlDeveloperPreset())">
                  {{ t('settings.fmtPresetPlsql') }}
                </el-button>
                <el-button size="small" @click="prefs.updateFormat(defaultFormatPrefs())">
                  {{ t('settings.fmtPresetDefault') }}
                </el-button>
              </span>
            </div>
            <div class="fmt">
              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupIndent') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtIndent') }}</span>
                    <el-input-number :model-value="prefs.formatPrefs.indent" :min="1" :max="8" size="small" controls-position="right" class="fmt-num" @update:model-value="(v: number | undefined) => prefs.updateFormat({ indent: v ?? 2 })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtUseTabs') }}</span>
                    <span class="fmt-ctrl">
                      <el-switch :model-value="prefs.formatPrefs.useTab" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ useTab: Boolean(v) })" />
                      <el-input-number :model-value="prefs.formatPrefs.tabSize" :min="1" :max="8" size="small" controls-position="right" class="fmt-num-sm" @update:model-value="(v: number | undefined) => prefs.updateFormat({ tabSize: v ?? 2 })" />
                    </span>
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtRightMargin') }}</span>
                    <el-input-number :model-value="prefs.formatPrefs.rightMargin" :min="40" :max="300" :step="10" size="small" controls-position="right" class="fmt-num" @update:model-value="(v: number | undefined) => prefs.updateFormat({ rightMargin: v ?? 90 })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtEmptyLines') }}</span>
                    <el-select :model-value="prefs.formatPrefs.emptyLines" size="small" class="fmt-select" @update:model-value="(v: number) => prefs.updateFormat({ emptyLines: v as 0 | 1 | 2 })">
                      <el-option :value="0" :label="t('settings.fmtEmptyRemove')" />
                      <el-option :value="1" :label="t('settings.fmtEmptyCollapse')" />
                      <el-option :value="2" :label="t('settings.fmtEmptyKeep')" />
                    </el-select>
                  </div>
                </div>
              </div>

              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupCase') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtKeywordCase') }}</span>
                    <el-select :model-value="prefs.formatPrefs.keywordCase" size="small" class="fmt-select" @update:model-value="(v: number) => prefs.updateFormat({ keywordCase: v as 0 | 1 | 2 | 3 })">
                      <el-option :value="0" :label="t('settings.fmtPreserve')" />
                      <el-option :value="1" :label="t('settings.fmtUpper')" />
                      <el-option :value="2" :label="t('settings.fmtLower')" />
                      <el-option :value="3" :label="t('settings.fmtCapitalize')" />
                    </el-select>
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtIdentCase') }}</span>
                    <el-select :model-value="prefs.formatPrefs.identifierCase" size="small" class="fmt-select" @update:model-value="(v: number) => prefs.updateFormat({ identifierCase: v as 0 | 1 | 2 | 3 })">
                      <el-option :value="0" :label="t('settings.fmtPreserve')" />
                      <el-option :value="1" :label="t('settings.fmtUpper')" />
                      <el-option :value="2" :label="t('settings.fmtLower')" />
                      <el-option :value="3" :label="t('settings.fmtCapitalize')" />
                    </el-select>
                  </div>
                </div>
              </div>

              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupItem') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtItemFormat') }}</span>
                    <el-select :model-value="prefs.formatPrefs.selectItemFormat" size="small" class="fmt-select" @update:model-value="(v: number) => prefs.updateFormat({ selectItemFormat: v as 0 | 1 | 2 | 3 })">
                      <el-option :value="0" :label="t('settings.fmtItemSameLine')" />
                      <el-option :value="1" :label="t('settings.fmtItemFit')" />
                      <el-option :value="2" :label="t('settings.fmtItemPerLine')" />
                      <el-option :value="3" :label="t('settings.fmtItemAdaptive')" />
                    </el-select>
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtItemAlign') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.selectItemAlign" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ selectItemAlign: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtCommaAfter') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.selectCommaAfter" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ selectCommaAfter: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtOnOneLine') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.onOneLineIfPossible" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ onOneLineIfPossible: Boolean(v) })" />
                  </div>
                </div>
              </div>

              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupClause') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtWhereSplitAndOr') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.whereSplitAndOr" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ whereSplitAndOr: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtAndOrAfter') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.andOrAfterExpression" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ andOrAfterExpression: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtAndOrUnderWhere') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.whereAndOrUnderWhere" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ whereAndOrUnderWhere: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtJoinSplitOn') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.joinSplitBeforeOn" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ joinSplitBeforeOn: Boolean(v) })" />
                  </div>
                </div>
              </div>

              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupControl') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtThenNewline') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.thenOnNewLine" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ thenOnNewLine: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtLoopNewline') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.loopOnNewLine" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ loopOnNewLine: Boolean(v) })" />
                  </div>
                </div>
              </div>

              <div class="fmt-group">
                <div class="fmt-title">{{ t('settings.fmtGroupAlign') }}</div>
                <div class="fmt-grid">
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtAlignDecl') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.alignDeclarationGroups" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ alignDeclarationGroups: Boolean(v) })" />
                  </div>
                  <div class="fmt-item">
                    <span class="fmt-label">{{ t('settings.fmtAlignAssign') }}</span>
                    <el-switch :model-value="prefs.formatPrefs.alignAssignmentGroups" @update:model-value="(v: string | number | boolean) => prefs.updateFormat({ alignAssignmentGroups: Boolean(v) })" />
                  </div>
                </div>
              </div>
            </div>
          </el-form>
        </template>

        <!-- 结果集 -->
        <template v-else-if="active === 'result'">
          <div class="pref-section">{{ t('settings.resultSection') }}</div>
          <el-form label-width="120px" size="small">
            <el-form-item :label="t('settings.resultRowHeight')">
              <el-radio-group
                :model-value="prefs.result.rowHeight"
                @update:model-value="(v: string | number | boolean | undefined) => prefs.updateResult({ rowHeight: (v as 'compact' | 'normal' | 'loose') ?? 'compact' })"
              >
                <el-radio-button value="compact">{{ t('settings.rowHeightCompact') }}</el-radio-button>
                <el-radio-button value="normal">{{ t('settings.rowHeightNormal') }}</el-radio-button>
                <el-radio-button value="loose">{{ t('settings.rowHeightLoose') }}</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.resultZebra')">
              <el-switch
                :model-value="prefs.result.zebra"
                @update:model-value="(v: string | number | boolean) => prefs.updateResult({ zebra: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.resultMaxRows')">
              <el-input-number
                :model-value="prefs.result.maxRows"
                :min="100"
                :max="100000"
                :step="100"
                controls-position="right"
                style="width: 150px"
                @update:model-value="(v: number | undefined) => prefs.updateResult({ maxRows: v ?? 1000 })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.resultPageSize')">
              <el-input-number
                :model-value="prefs.result.pageSize"
                :min="1"
                :max="100"
                controls-position="right"
                style="width: 150px"
                @update:model-value="(v: number | undefined) => prefs.updateResult({ pageSize: Math.min(100, Math.max(1, v ?? 10)) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.resultMaxTotalRows')">
              <el-input-number
                :model-value="prefs.result.maxTotalRows"
                :min="100"
                :max="10000"
                :step="100"
                controls-position="right"
                style="width: 150px"
                @update:model-value="(v: number | undefined) => prefs.updateResult({ maxTotalRows: Math.min(10000, Math.max(100, v ?? 10000)) })"
              />
            </el-form-item>

            <div class="pref-section">{{ t('settings.exportSection') }}</div>
            <el-form-item :label="t('settings.expUnquoteIdent')">
              <el-switch
                :model-value="prefs.exportPrefs.unquoteIdent"
                @update:model-value="(v: string | number | boolean) => prefs.updateExport({ unquoteIdent: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.expIdentCase')">
              <el-radio-group
                :model-value="prefs.exportPrefs.identCase"
                @update:model-value="(v: 'original' | 'upper' | 'lower') => prefs.updateExport({ identCase: v })"
              >
                <el-radio-button value="original">{{ t('settings.expIdentOriginal') }}</el-radio-button>
                <el-radio-button value="upper">{{ t('settings.expIdentUpper') }}</el-radio-button>
                <el-radio-button value="lower">{{ t('settings.expIdentLower') }}</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.expDateConvert')">
              <el-switch
                :model-value="prefs.exportPrefs.dateConvert"
                @update:model-value="(v: string | number | boolean) => prefs.updateExport({ dateConvert: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item label="UTF-8 BOM">
              <el-switch
                :model-value="prefs.exportPrefs.bom"
                @update:model-value="(v: string | number | boolean) => prefs.updateExport({ bom: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.expBatchSize')">
              <el-input-number
                :model-value="prefs.exportPrefs.insertBatchSize"
                :min="1"
                :max="1000"
                controls-position="right"
                style="width: 150px"
                @update:model-value="(v: number | undefined) => prefs.updateExport({ insertBatchSize: Math.min(1000, Math.max(1, v ?? 10)) })"
              />
              <span class="field-hint">{{ t('settings.expBatchHint') }}</span>
            </el-form-item>
            <el-form-item :label="t('settings.expValueRules')">
              <ValueRuleEditor
                :model-value="prefs.exportPrefs.valueRules"
                @update:model-value="(r) => prefs.updateExport({ valueRules: r })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.resultFormatJson')">
              <el-switch
                :model-value="prefs.result.formatJson"
                @update:model-value="(v: string | number | boolean) => prefs.updateResult({ formatJson: Boolean(v) })"
              />
            </el-form-item>
          </el-form>
        </template>

        <!-- 查看器（源码查看器增强项 + 实时预览） -->
        <template v-else-if="active === 'viewer'">
          <div class="pref-section">{{ t('settings.viewerSection') }}</div>
          <el-form label-width="120px" size="small">
            <el-form-item :label="t('settings.viewerLineNumbers')">
              <el-switch
                :model-value="prefs.viewer.lineNumbers"
                @update:model-value="(v: string | number | boolean) => prefs.updateViewer({ lineNumbers: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.viewerFolding')">
              <el-switch
                :model-value="prefs.viewer.folding"
                @update:model-value="(v: string | number | boolean) => prefs.updateViewer({ folding: Boolean(v) })"
              />
            </el-form-item>
          </el-form>
          <div class="pref-section">{{ t('settings.viewerPreview') }}</div>
          <div class="viewer-preview">
            <CodeViewer :model-value="previewSql" kind="mysql" />
          </div>
        </template>

        <!-- 数据格式 -->
        <template v-else-if="active === 'dataformat'">
          <div class="pref-section">{{ t('settings.dataFormatSection') }}</div>
          <el-form label-width="120px" size="small">
            <el-form-item :label="t('settings.dfDateStyle')">
              <el-radio-group
                :model-value="prefs.dataFormat.dateStyle"
                @update:model-value="(v: string | number | boolean | undefined) => prefs.updateDataFormat({ dateStyle: (v as 'original' | 'local' | 'iso') ?? 'original' })"
              >
                <el-radio-button value="original">{{ t('settings.dfDateOriginal') }}</el-radio-button>
                <el-radio-button value="local">{{ t('settings.dfDateLocal') }}</el-radio-button>
                <el-radio-button value="iso">ISO 8601</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.dfThousand')">
              <el-switch
                :model-value="prefs.dataFormat.thousand"
                @update:model-value="(v: string | number | boolean) => prefs.updateDataFormat({ thousand: Boolean(v) })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.dfBoolStyle')">
              <el-radio-group
                :model-value="prefs.dataFormat.boolStyle"
                @update:model-value="(v: string | number | boolean | undefined) => prefs.updateDataFormat({ boolStyle: (v as 'trueFalse' | 'yesNo') ?? 'trueFalse' })"
              >
                <el-radio-button value="trueFalse">true / false</el-radio-button>
                <el-radio-button value="yesNo">YES / NO</el-radio-button>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="t('settings.dfNullText')">
              <el-input
                :model-value="prefs.dataFormat.nullText"
                style="width: 130px"
                @update:model-value="(v: string) => prefs.updateDataFormat({ nullText: v })"
              />
            </el-form-item>
            <el-form-item :label="t('settings.dfTruncate')">
              <el-input-number
                :model-value="prefs.dataFormat.truncateLength"
                :min="20"
                :max="5000"
                :step="50"
                controls-position="right"
                style="width: 130px"
                @update:model-value="(v: number | undefined) => prefs.updateDataFormat({ truncateLength: v ?? 300 })"
              />
            </el-form-item>
          </el-form>
          <div class="pref-section">{{ t('settings.dfPreview') }}</div>
          <el-table :data="dfPreviewRows" size="small" border class="df-preview">
            <el-table-column prop="type" :label="t('settings.dfPreviewType')" width="110" />
            <el-table-column prop="raw" :label="t('cell.raw')" min-width="150" />
            <el-table-column :label="t('settings.dfPreviewShown')" min-width="180">
              <template #default="{ row }">
                <CellDisplay :value="row.value" :semantic="row.semantic" />
              </template>
            </el-table-column>
          </el-table>
        </template>

        <!-- 存储 -->
        <template v-else-if="active === 'storage'">
          <div class="pref-section">{{ t('settings.storageSection') }}</div>
          <el-alert
            :title="t('settings.storageHint')"
            type="info"
            show-icon
            :closable="false"
            class="storage-hint"
          />
          <div v-for="d in storageDirs" :key="d.key" class="storage-card">
            <div class="storage-head">
              <span class="storage-name">{{ t(d.labelKey) }}</span>
              <el-tag v-if="d.isCustom" size="small" type="warning">{{ t('settings.storageCustom') }}</el-tag>
              <el-tag v-if="d.key === 'logs'" size="small" type="info">
                {{ t('settings.storageRestartHint') }}
              </el-tag>
            </div>
            <div class="storage-path" :title="d.dir">{{ d.dir }}</div>
            <div class="storage-actions">
              <el-button size="small" @click="chooseStorageDir(d.key)">
                {{ t('settings.storageModify') }}
              </el-button>
              <el-button size="small" :disabled="!d.isCustom" @click="resetStorageDir(d.key)">
                {{ t('settings.storageReset') }}
              </el-button>
              <el-button size="small" :icon="FolderOpened" @click="openStorageDir(d.key)">
                {{ t('settings.storageOpen') }}
              </el-button>
            </div>
          </div>
        </template>

        <!-- 快捷键 -->
        <template v-else-if="active === 'shortcuts'">
          <div class="pref-section">{{ t('settings.shortcutsSection') }}</div>
          <div class="shortcut-tools">
            <el-button size="small" :icon="RefreshLeft" @click="prefs.resetAllShortcuts()">
              {{ t('settings.shortcutResetAll') }}
            </el-button>
            <span class="shortcut-hint">{{ t('settings.shortcutHint') }}</span>
          </div>
          <el-table :data="shortcutRows" size="small" border max-height="300">
            <el-table-column :label="t('settings.shortcutAction')" min-width="150">
              <template #default="{ row }">
                <span>{{ t(row.labelKey) }}</span>
                <el-tag size="small" type="info" class="shortcut-scope">
                  {{ row.scope === 'global' ? t('settings.shortcutGlobal') : t('settings.shortcutEditor') }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column :label="t('settings.shortcutBinding')" width="200">
              <template #default="{ row }">
                <el-input
                  :model-value="recordingKey === row.key ? recordingText : prefs.shortcutOf(row.key)"
                  size="small"
                  readonly
                  :placeholder="t('settings.shortcutPress')"
                  :class="{ 'is-recording': recordingKey === row.key }"
                  @focus="startRecording(row.key)"
                  @blur="stopRecording"
                  @keydown="onRecordKeydown($event, row.key)"
                />
              </template>
            </el-table-column>
            <el-table-column :label="t('snippets.actions')" width="80" align="center">
              <template #default="{ row }">
                <el-button
                  size="small"
                  text
                  :icon="RefreshLeft"
                  :disabled="!prefs.shortcuts[row.key]"
                  @click="prefs.resetShortcut(row.key)"
                />
              </template>
            </el-table-column>
          </el-table>
        </template>

        <!-- 关于 -->
        <template v-else-if="active === 'about'">
          <div class="pref-section">{{ t('settings.tabAbout') }}</div>
          <div class="about-block">
            <div class="about-name">PersonalDb</div>
            <div class="about-version">{{ t('menu.version') }} 0.1.0-beta.2</div>
            <div class="about-desc">{{ t('menu.aboutBody') }}</div>
            <div class="about-stack">Tauri 2 · WebView2 · Vue 3 · TypeScript · Rust</div>
          </div>

          <!-- 更新提示（查 GitHub Releases，无自动检测；规格见 docs/specifications.md SPEC-DEV-2） -->
          <div class="pref-section">{{ t('settings.updateSection') }}</div>
          <el-form label-width="140px" size="small">
            <el-form-item v-if="prefs.update.skippedVersion" :label="t('settings.updateSkipped')">
              <span class="update-skipped">{{ prefs.update.skippedVersion }}</span>
              <el-button size="small" link type="primary" @click="prefs.clearSkippedVersion()">
                {{ t('settings.updateClearSkipped') }}
              </el-button>
            </el-form-item>
            <el-form-item :label="t('settings.updateStatus')">
              <el-button size="small" :loading="updateStore.busy" @click="updateStore.runManualCheck()">
                {{ t('update.check') }}
              </el-button>
              <el-button
                v-if="updateStore.hasUpdate"
                size="small"
                link
                type="primary"
                @click="updateStore.openRelease()"
              >
                {{ t('update.openRelease') }}
              </el-button>
              <span v-if="updateStore.hasUpdate" class="update-hint">
                {{ t('update.availableHint', { version: updateStore.version }) }}
              </span>
              <span v-else-if="updateStore.outcome === 'latest'" class="update-hint">
                {{ t('update.latest') }}
              </span>
              <span v-else-if="updateStore.outcome === 'error'" class="update-hint">
                {{ t('update.checkFailed', { msg: updateStore.lastError }) }}
              </span>
            </el-form-item>
          </el-form>
        </template>
      </main>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import {
  FolderOpened,
  RefreshLeft,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import CodeViewer from './CodeViewer.vue'
import CellDisplay from './cell-viewers/CellDisplay.vue'
import ValueRuleEditor from './ValueRuleEditor.vue'
import { plsqlDeveloperPreset, defaultFormatPrefs } from '../stores/preferences'
import { usePreferencesStore } from '../stores/preferences'
import { useUpdateStore } from '../stores/update'
import { SHORTCUT_ACTIONS, eventToCombo } from '../utils/shortcuts'
import {
  storageInfo,
  storageOpenDir,
  storageResetDir,
  storageSetDir,
  type StorageDirInfo,
} from '../api/db'
import type { CellValue, SemanticType } from '../types/db'

defineProps<{ modelValue: boolean }>()

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const { t } = useI18n()
const prefs = usePreferencesStore()
const updateStore = useUpdateStore()

const active = ref('appearance')

/** 数据格式预览行（各类型示例值） */
const dfPreviewRows: Array<{ type: string; raw: string; value: CellValue; semantic: SemanticType }> = [
  { type: 'DATETIME', raw: '2026-10-12 08:30:00', value: '2026-10-12 08:30:00', semantic: 'datetime' },
  { type: 'DECIMAL', raw: '1234567.891', value: '1234567.891', semantic: 'decimal' },
  { type: 'BOOLEAN', raw: 'true', value: true, semantic: 'bool' },
  { type: 'VARCHAR', raw: 'x'.repeat(420), value: 'x'.repeat(420), semantic: 'text' },
  { type: 'NULL', raw: 'NULL', value: null, semantic: 'text' },
]

/** 快捷键录制状态 */
const recordingKey = ref('')
const recordingText = ref('')

const shortcutRows = computed(() =>
  SHORTCUT_ACTIONS.map((a) => ({ key: a.key, labelKey: a.labelKey, scope: a.scope })),
)

function startRecording(key: string): void {
  recordingKey.value = key
  recordingText.value = ''
}

function stopRecording(): void {
  recordingKey.value = ''
  recordingText.value = ''
}

function onRecordKeydown(e: KeyboardEvent, key: string): void {
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') {
    stopRecording()
    return
  }
  const combo = eventToCombo(e)
  if (!combo) {
    // 仅修饰键：实时展示当前组合
    const parts: string[] = []
    if (e.ctrlKey) parts.push('Ctrl')
    if (e.shiftKey) parts.push('Shift')
    if (e.altKey) parts.push('Alt')
    recordingText.value = parts.join('+')
    return
  }
  prefs.setShortcut(key, combo)
  ElMessage.success(t('settings.shortcutSaved', { combo }))
  stopRecording()
}

/** 源码查看器预览示例（含括号代码块便于观察折叠效果） */
const previewSql = `CREATE TABLE demo_orders (
  id BIGINT NOT NULL,
  order_no VARCHAR(64) NOT NULL,
  customer_id BIGINT,
  amount DECIMAL(18, 2) DEFAULT 0.00,
  status VARCHAR(16) DEFAULT 'NEW',
  remark VARCHAR(500),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uk_order_no (order_no),
  KEY idx_customer (customer_id, status)
);`

const categories = computed(() => [
  { key: 'appearance', label: t('settings.appearance'), icon: objectIcon('act_theme') },
  { key: 'editor', label: t('settings.editorSection'), icon: objectIcon('act_code') },
  { key: 'viewer', label: t('settings.viewerSection'), icon: objectIcon('act_code_view') },
  { key: 'result', label: t('settings.resultSection'), icon: objectIcon('act_view_grid') },
  { key: 'dataformat', label: t('settings.dataFormatSection'), icon: objectIcon('act_format') },
  { key: 'storage', label: t('settings.storageSection'), icon: objectIcon('obj_folder') },
  { key: 'shortcuts', label: t('settings.shortcutsSection'), icon: objectIcon('act_keyboard') },
  { key: 'about', label: t('settings.tabAbout'), icon: objectIcon('act_info') },
])

/** —— 存储类别 —— */
const storageDirs = ref<StorageDirInfo[]>([])

async function reloadStorage(): Promise<void> {
  try {
    storageDirs.value = await storageInfo()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

void reloadStorage()

async function chooseStorageDir(key: string): Promise<void> {
  try {
    const dir = await openFileDialog({ directory: true })
    if (!dir) return
    const list = await storageSetDir(key, dir)
    storageDirs.value = list
    ElMessage.success(t('settings.storageMoved'))
  } catch (e) {
    ElMessage.error(t('settings.storageMoveFailed', { msg: String(e) }))
  }
}

async function resetStorageDir(key: string): Promise<void> {
  try {
    const list = await storageResetDir(key)
    storageDirs.value = list
    ElMessage.success(t('settings.storageResetDone'))
  } catch (e) {
    ElMessage.error(String(e))
  }
}

function openStorageDir(key: string): void {
  void storageOpenDir(key)
    .then((path) => revealItemInDir(path))
    .catch((e) => ElMessage.error(String(e)))
}
</script>

<style scoped>
.settings-layout {
  display: flex;
  gap: 16px;
  height: 440px;
}

.settings-nav {
  width: 148px;
  flex-shrink: 0;
  border-right: 1px solid var(--el-border-color-lighter);
  padding-right: 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
.nav-item :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}

  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--el-text-color-regular);
  cursor: pointer;
  user-select: none;
}

.nav-item:hover {
  background: var(--el-fill-color-light);
}

.nav-item.is-active {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  font-weight: 600;
}

.settings-content {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding-right: 6px;
}

.pref-section {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 12px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.field-hint {
  margin-left: 8px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.preset-actions {
  float: right;
  display: inline-flex;
  gap: 6px;
  text-transform: none;
  letter-spacing: normal;
}

.fmt {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.fmt-group {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 10px 14px;
  background: var(--el-fill-color-blank);
}

.fmt-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}

.fmt-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 22px;
}

.fmt-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 30px;
}

.fmt-label {
  font-size: 13px;
  color: var(--el-text-color-regular);
}

.fmt-ctrl {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.fmt-num {
  width: 100px;
}

.fmt-num-sm {
  width: 84px;
}

.fmt-select {
  width: 140px;
}

.about-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.about-name {
  font-size: 20px;
  font-weight: 700;
  color: var(--el-color-primary);
}

.about-version {
  font-size: 13px;
  color: var(--el-text-color-regular);
}

.about-desc {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.about-stack {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  font-family: Consolas, monospace;
}

/* 更新提示：状态提示与已跳过版本 */
.update-hint {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.update-skipped {
  margin-right: 8px;
  font-size: 12px;
  font-family: Consolas, monospace;
  color: var(--el-text-color-regular);
}

.viewer-preview {
  height: 260px;
}

.shortcut-tools {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}

.shortcut-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.shortcut-scope {
  margin-left: 8px;
}

.is-recording :deep(.el-input__wrapper) {
  box-shadow: 0 0 0 1px var(--el-color-primary) inset;
}

/* 存储类别卡片 */
.storage-hint {
  margin-bottom: 12px;
}

.storage-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 10px 12px;
  margin-bottom: 10px;
}

.storage-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.storage-name {
  font-weight: 600;
  font-size: 13px;
}

.storage-path {
  margin: 6px 0;
  font-size: 12px;
  font-family: Consolas, monospace;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.storage-actions {
  display: flex;
  gap: 8px;
}
</style>