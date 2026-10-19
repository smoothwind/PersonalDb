<template>
  <div class="query-pane">
    <div class="query-toolbar" data-tour="query-toolbar">
      <el-select
        :model-value="tab.database"
        :placeholder="t('query.defaultDb')"
        clearable
        filterable
        size="small"
        class="db-select"
        @update:model-value="tab.database = $event ?? ''"
      >
        <el-option v-for="d in databases" :key="d" :label="d" :value="d" />
      </el-select>
      <el-divider direction="vertical" />
      <template v-if="!tab.running">
        <el-tooltip :content="t('query.runAllTip')" placement="bottom">
          <el-button
            :aria-label="t('query.runAllTip')"
            type="primary"
            size="small"
            circle
            :disabled="!tab.sql.trim()"
            @click="run('auto')"
          >
            <el-icon><component :is="objectIcon('act_run')" /></el-icon>
          </el-button>
        </el-tooltip>
        <el-tooltip :content="t('query.runSelectedTip')" placement="bottom">
          <el-button
            :aria-label="t('query.runSelectedTip')"
            size="small"
            circle
            :disabled="!tab.sql.trim()"
            @click="run('selection')"
          >
            <el-icon><component :is="objectIcon('act_run')" /></el-icon>
          </el-button>
        </el-tooltip>
        <el-tooltip :content="t('query.runLineTip')" placement="bottom">
          <el-button
            :aria-label="t('query.runLineTip')"
            size="small"
            circle
            :disabled="!tab.sql.trim()"
            @click="run('line')"
          >
            <el-icon><component :is="objectIcon('act_run')" /></el-icon>
          </el-button>
        </el-tooltip>
      </template>
      <el-tooltip v-if="tab.running" :content="t('query.stopTip')" placement="bottom">
        <el-button
            :aria-label="t('query.stopTip')" type="danger" size="small" circle @click="tabsStore.cancelRun(tab)">
          <el-icon><component :is="objectIcon('act_stop')" /></el-icon>
        </el-button>
      </el-tooltip>
      <el-divider direction="vertical" />
      <el-tooltip :content="t('query.snippets')" placement="bottom">
        <el-button
            :aria-label="t('query.snippets')" size="small" circle :icon="MagicStick" @click="snippetSettingsVisible = true" />
      </el-tooltip>
      <el-tooltip :content="t('query.formatTip')" placement="bottom">
        <el-button
            :aria-label="t('query.formatTip')" size="small" circle :icon="Brush" :disabled="!tab.sql.trim()" @click="doFormat" />
      </el-tooltip>
      <el-tooltip :content="t('query.compressTip')" placement="bottom">
        <el-button
            :aria-label="t('query.compressTip')"
          size="small"
          circle
          :icon="ScaleToOriginal"
          :disabled="!tab.sql.trim()"
          @click="doCompress"
        />
      </el-tooltip>
      <el-tooltip v-if="explainCap" :content="t('query.explainTip')" placement="bottom">
        <el-button
            :aria-label="t('query.explainTip')"
          size="small"
          circle
          :icon="DataAnalysis"
          :loading="explaining"
          :disabled="!tab.sql.trim()"
          @click="runExplain"
        />
      </el-tooltip>
      <el-divider direction="vertical" />
      <el-tooltip :content="t('script.save')" placement="bottom">
        <el-button
            :aria-label="t('script.save')" size="small" circle :disabled="!tab.sql.trim()" @click="saveSqlFile">
          <el-icon><component :is="objectIcon('act_save')" /></el-icon>
        </el-button>
      </el-tooltip>
      <el-tooltip :content="t('script.open')" placement="bottom">
        <el-button
            :aria-label="t('script.open')" size="small" circle :icon="FolderOpened" @click="openSqlFile" />
      </el-tooltip>
      <el-tooltip v-if="!connected" :content="t('conn.reconnect')" placement="bottom">
        <el-button
            :aria-label="t('conn.reconnect')" size="small" circle type="warning" @click="doReconnect">
          <el-icon><component :is="objectIcon('act_reconnect')" /></el-icon>
        </el-button>
      </el-tooltip>
      <HistoryPanel
        :conn-name="tab.connectionName"
        @apply="(sql: string) => (tab.sql = sql)"
        @run="runFromHistory"
      />
      <span class="hint">{{ t('dbKind.' + tab.kind) }}</span>
    </div>

    <!-- 事务管理已移至主窗体工具栏（见 App.vue） -->

    <div class="editor-wrap" :style="{ height: editorWrapHeight + 'px' }">
      <SqlEditor
        ref="editorRef"
        v-model="tab.sql"
        :kind="tab.kind"
        :snippets="snippetMap"
        :connection-id="tab.connectionId"
        :database="tab.database"
        @run="run"
        @format="doFormat"
      />
    </div>
    <div class="splitter-h" @mousedown="startEditorDrag" />

    <div class="result-wrap" data-tour="result">
      <el-alert
        v-if="tab.error"
        type="error"
        show-icon
        :closable="false"
        class="error-bar"
      >
        <template #title>
          <div class="eb-row">
            <span class="eb-text">{{ tab.error }}</span>
            <span class="eb-actions">
              <el-button
                v-if="errorIsTimeout"
                size="small"
                type="warning"
                plain
                @click="openTimeoutSettings"
              >
                {{ t('query.adjustTimeout') }}
              </el-button>
              <el-button
                v-if="errorIsConn"
                size="small"
                type="warning"
                plain
                @click="doReconnect"
              >
                {{ t('conn.reconnect') }}
              </el-button>
              <el-button v-else size="small" type="primary" plain @click="retryRun">
                {{ t('result.retry') }}
              </el-button>
              <el-button size="small" text @click="clearError">
                {{ t('common.close') }}
              </el-button>
            </span>
          </div>
        </template>
      </el-alert>
      <template v-else-if="tab.result">
        <div class="result-meta">
          <template v-if="tab.result.columns.length > 0">
            <!-- 段1：视图切换（多结果集 + 表格/记录/计划） -->
            <el-segmented
              v-if="resultCount > 1"
              :model-value="tab.activeResultIndex ?? 0"
              :options="resultOptions"
              size="small"
              class="multi-switch"
              @update:model-value="(v: number | string) => switchResult(Number(v))"
            />
            <el-segmented
              v-model="viewMode"
              size="small"
              :options="viewOptions"
              class="view-switch"
            />
            <span class="pt-sep" />
            <!-- 段2：统计与过滤 -->
            <el-tag size="small" type="info">
              {{ t('common.rows', { n: tab.result.rows.length }) }}
              <template v-if="tab.result.truncated">
                （{{ t('query.truncatedHint', { n: prefs.result.maxRows }) }}）
              </template>
            </el-tag>
            <el-input
              v-if="viewMode === 'grid' && !editMode"
              ref="searchInputRef"
              v-model="searchText"
              size="small"
              clearable
              :prefix-icon="Search"
              :placeholder="t('result.search')"
              class="result-search"
            />
            <span v-if="searchText && viewMode === 'grid'" class="match-count">
              {{ t('result.matchRows', { n: filteredIdx.length }) }}
            </span>
            <span v-if="activeFilterCount > 0 && viewMode === 'grid'" class="match-count">
              {{ t('result.filtered', { n: activeFilterCount }) }}
            </span>
            <span class="pt-spacer" />
            <!-- 段3：动作（统计/编辑/列/导出） -->
            <div class="meta-actions">
              <!-- 统计行开关 -->
              <el-button
                v-if="summaryCols.length > 0 && !editMode"
                size="small"
                circle
                :type="showSummary ? 'primary' : 'default'"
                :icon="TrendCharts"
                :title="t('result.summary')"
                @click="showSummary = !showSummary"
              />
              <!-- 编辑开关 -->
              <el-button
                v-if="tab.result && tab.result.columns.length > 0"
                size="small"
                circle
                :type="editMode ? 'warning' : 'default'"
                :icon="EditPen"
                :title="t('edit.toggle')"
                @click="toggleEdit"
              />
              <!-- 勾选展示列（子组件内暂存与拖拽排序，确认后父级一次重绘） -->
              <ColVisPopover
                :cols="orderedAllCols"
                :hidden="hiddenProps"
                @apply="applyColVis"
                @reorder="onColsReorder"
                @reset="resetCols"
              />
              <!-- 选中项导出 -->
              <el-dropdown trigger="click" @command="onExportRows">
                <el-button
                  size="small"
                  circle
                  :title="t('result.exportSelected')"
                  :disabled="filteredIdx.length === 0"
                >
                  <el-icon><component :is="objectIcon('act_export')" /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="excel">
                      Excel{{ selectedRows.length ? `（${selectedRows.length}）` : '' }}
                    </el-dropdown-item>
                    <el-dropdown-item command="sql">
                      SQL INSERT{{ selectedRows.length ? `（${selectedRows.length}）` : '' }}
                    </el-dropdown-item>
                    <el-dropdown-item command="csv">
                      CSV{{ selectedRows.length ? `（${selectedRows.length}）` : '' }}
                    </el-dropdown-item>
                    <el-dropdown-item command="json">
                      JSON{{ selectedRows.length ? `（${selectedRows.length}）` : '' }}
                    </el-dropdown-item>
                    <el-dropdown-item command="txt">
                      TXT{{ selectedRows.length ? `（${selectedRows.length}）` : '' }}
                    </el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
          </template>
          <template v-else>
            <el-tag size="small" type="success">
              <template v-if="tab.result.affectedRows != null">
                {{ t('query.successAffected', { n: tab.result.affectedRows }) }}
              </template>
              <template v-else>{{ t('query.successNoResult') }}</template>
            </el-tag>
            <span class="pt-spacer" />
          </template>
          <el-tag v-if="tab.result.durationMs > 0" size="small">{{ tab.result.durationMs }} ms</el-tag>
          <span v-if="tab.database" class="ctx-tag">@{{ tab.database }}</span>
        </div>

        <PlanTree
          v-if="viewMode === 'plan' && planData"
          :root="planData"
          class="plan-wrap"
        />
        <RecordView
          v-else-if="tab.result.columns.length > 0 && viewMode === 'record'"
          :key="'rec-' + recordStart"
          :columns="columns"
          :rows="resultRows"
          :start-index="recordStart"
          @detail="onDetail"
        />
        <template v-else-if="tab.result.columns.length > 0">
          <div v-if="editMode" class="edit-strip" :class="{ 'is-risk': editHighRisk }">
            <span class="es-text">
              {{ t('edit.activeStrip', { table: editTable, mode: editModeLabel }) }}
            </span>
            <el-button size="small" text type="danger" @click="exitEdit">
              {{ t('edit.exit') }}
            </el-button>
          </div>
          <ResultVirtualGrid
            v-if="useVirtualGrid"
            :columns="virtualColumns"
            :rows="virtualRows"
            @cell-menu="onVirtualCellMenu"
            @row-copy="onVirtualRowCopy"
          />
          <el-table
            v-else
            ref="tableRef"
          :data="editMode ? editData : resultRows"
          border
          :stripe="prefs.result.zebra"
          size="small"
          class="result-table"
          :class="{ 'is-compact': isCompact, 'is-editing': editMode }"
          :row-style="{ height: rowHeight }"
          :row-class-name="editRowClass"
          @cell-click="onCellClick"
          @cell-contextmenu="onCellContextMenu"
          @cell-mouse-enter="onCellMouseEnter"
          @cell-mouse-leave="onCellMouseLeave"
          @header-click="onHeaderClick"
          @header-dragend="(_w: number, _o: number, column: { property?: string }) => onHeaderResize(column.property, _w)"
          @row-click="onRowClick"
          @selection-change="onSelectionChange"
        >
          <el-table-column type="selection" width="38" fixed />
          <el-table-column type="index" width="48" fixed />
          <el-table-column
            v-for="col in columns"
            :key="col.prop"
            :prop="col.prop"
            :label="col.name"
            :min-width="colWidths[col.prop] || 140"
            :width="colWidths[col.prop] || undefined"
            :align="col.align"
            :fixed="col.fixed"
            :sortable="true"
            show-overflow-tooltip
          >
            <template #header>
                <div class="col-head">
                <div class="col-title">
                  <span class="col-name" :title="`${col.name}${col.typeLabel ? ' · ' + col.typeLabel : ''}`">{{ col.name }}</span>
                  <el-popover trigger="click" :width="252" placement="bottom-start">
                    <template #reference>
                      <el-icon
                        class="col-fmt-btn"
                        :class="{ 'is-set': hasColSetting(col.prop) }"
                        :title="t('cell.colFormatTip')"
                        @click.stop
                      >
                        <Setting />
                      </el-icon>
                    </template>
                    <div class="col-settings" @click.stop>
                      <div class="cs-row">
                        <span class="cs-label">{{ t('cell.colFormatTip') }}</span>
                        <el-select
                          :model-value="colSettings[col.prop]?.format ?? 'raw'"
                          size="small"
                          style="width: 120px"
                          @update:model-value="(v: CellFormat) => setColOption(col.prop, 'format', v === 'raw' ? undefined : v)"
                        >
                          <el-option value="raw" :label="t('cell.formatRaw')" />
                          <el-option v-if="col.semantic === 'json'" value="pretty" :label="t('cell.formatPretty')" />
                          <el-option v-if="isDateLike(col.semantic)" value="local" :label="t('cell.formatLocal')" />
                          <el-option v-if="isDateLike(col.semantic)" value="iso" :label="t('cell.formatIso')" />
                          <el-option v-if="isNumberLike(col.semantic)" value="thousand" :label="t('cell.formatThousand')" />
                          <el-option v-if="col.semantic === 'longtext' || col.semantic === 'text'" value="full" :label="t('cell.formatFull')" />
                        </el-select>
                      </div>
                      <div class="cs-row">
                        <span class="cs-label">{{ t('result.fixed') }}</span>
                        <el-radio-group
                          :model-value="colSettings[col.prop]?.fixed ?? ''"
                          size="small"
                          @update:model-value="(v: string | number | boolean | undefined) => setColOption(col.prop, 'fixed', v ? (v as 'left' | 'right') : undefined)"
                        >
                          <el-radio-button value="">{{ t('result.fixedNone') }}</el-radio-button>
                          <el-radio-button value="left">{{ t('result.fixedLeft') }}</el-radio-button>
                          <el-radio-button value="right">{{ t('result.fixedRight') }}</el-radio-button>
                        </el-radio-group>
                      </div>
                      <div class="cs-row">
                        <span class="cs-label">{{ t('result.filter') }}</span>
                        <el-input
                          :model-value="colSettings[col.prop]?.filter ?? ''"
                          size="small"
                          clearable
                          style="width: 120px"
                          :placeholder="t('result.filterPlaceholder')"
                          @change="(v: string) => commitColFilter(col.prop, v)"
                          @clear="commitColFilter(col.prop, '')"
                        />
                      </div>
                      <div class="cs-row">
                        <span class="cs-label">{{ t('result.moveCol') }}</span>
                        <el-button-group size="small">
                          <el-button size="small" @click="moveColEdge(col.prop, 'front')">
                            {{ t('result.moveFirst') }}
                          </el-button>
                          <el-button size="small" @click="moveColEdge(col.prop, 'back')">
                            {{ t('result.moveLast') }}
                          </el-button>
                        </el-button-group>
                      </div>
                      <ColValueFilter
                        :values="distinctOf(col.prop)"
                        :model-value="valueFilters[col.prop] ?? []"
                        @update:model-value="(vals: string[]) => setValueFilter(col.prop, vals)"
                      />
                    </div>
                  </el-popover>
                </div>
                <span v-if="col.typeLabel" class="col-type">{{ col.typeLabel }}</span>
              </div>
            </template>
            <template #default="{ row, $index }">
              <div v-if="editMode" class="cell-wrap" @dblclick="beginEdit($index, col.prop)">
                <el-input
                  v-if="editingCell && editingCell.rid === $index && editingCell.prop === col.prop"
                  v-focus
                  v-model="editValue"
                  size="small"
                  @keyup.enter="commitEdit"
                  @keyup.esc="cancelEdit"
                  @blur="commitEdit"
                />
                <span v-else class="edit-cell">{{ row[col.prop] == null ? '' : String(row[col.prop]) }}</span>
              </div>
              <div v-else class="cell-wrap" @dblclick="onDetail({ col, value: row[col.prop] })">
                <CellDisplay
                  :value="row[col.prop]"
                  :semantic="col.semantic"
                  :format="col.format"
                />
              </div>
            </template>
          </el-table-column>
        </el-table>
        <div v-if="showSummary && summaryData.length > 0 && viewMode === 'grid'" class="sum-bar">
          <el-tag size="small" type="info" class="sum-chip">
            {{ t('common.rows', { n: filteredIdx.length }) }}
          </el-tag>
          <span
            v-for="s in summaryData"
            :key="s.prop"
            class="sum-chip"
            :title="`${t('result.statMin')}: ${s.min} · ${t('result.statMax')}: ${s.max}`"
          >
            <span class="sum-name">{{ s.name }}</span>
            <span class="sum-val">Σ {{ formatSum(s.sum) }} · x̄ {{ formatSum(s.avg) }}</span>
          </span>
        </div>
        <div v-if="tab.loadingMore || tab.hasMore || reachLimit" class="load-more-hint">
          <template v-if="tab.loadingMore">{{ t('common.loading') }}</template>
          <template v-else-if="reachLimit">
            {{ t('result.limitReached', { n: prefs.result.maxTotalRows }) }}
          </template>
          <template v-else>
            <span>{{ t('result.scrollMore') }}</span>
            <el-button size="small" text type="primary" @click="tabsStore.loadMore(tab)">
              {{ t('result.loadMoreNow') }}
            </el-button>
          </template>
        </div>
        <div v-if="editMode" class="edit-bar">
          <span>
            {{ t('edit.pending', { ins: editCounts.ins, upd: editCounts.upd, del: editCounts.del }) }}
          </span>
          <span v-if="editHighRisk" class="edit-risk">{{ t('edit.highRisk') }}</span>
          <div class="spacer" />
          <el-button size="small" @click="exitEdit">{{ t('edit.discard') }}</el-button>
          <el-button
            size="small"
            type="primary"
            :disabled="editCounts.total === 0"
            @click="openPreview"
          >
            {{ t('edit.preview') }}
          </el-button>
          <el-tooltip :content="t('edit.undo')" placement="bottom">
            <el-button
            :aria-label="t('edit.undo')"
              size="small"
              circle
              :disabled="editUndoStack.length === 0"
              @click="undoGridEdit"
            >
              <el-icon><component :is="objectIcon('act_undo')" /></el-icon>
            </el-button>
          </el-tooltip>
          <el-tooltip :content="t('edit.redo')" placement="bottom">
            <el-button
            :aria-label="t('edit.redo')"
              size="small"
              circle
              :disabled="editRedoStack.length === 0"
              @click="redoGridEdit"
            >
              <el-icon><component :is="objectIcon('act_redo')" /></el-icon>
            </el-button>
          </el-tooltip>
        </div>
        </template>
        <EmptyState
          v-else
          size="compact"
          icon="obj_table"
          :title="t('query.successNoResult')"
        />
      </template>
      <EmptyState
        v-else
        icon="obj_query"
        :title="t('query.resultPlaceholder')"
        :hint="t('query.resultPlaceholderHint')"
      />
    </div>

    <CellDetailDrawer v-model="detailVisible" :detail="detailState" />

    <!-- 长文本/JSON 悬浮卡 -->
    <el-popover
      :visible="cellCardVisible"
      :virtual-ref="cellCardAnchor"
      virtual-triggering
      placement="top-start"
      :width="460"
      popper-class="sql-hover-popper"
      @hide="cellCardVisible = false"
    >
      <div class="sql-card">
        <div class="sql-card-head">
          <span class="sql-card-title">{{ t('cell.preview') }}</span>
          <el-button size="small" text type="primary" @click="copyCellCard">
            {{ t('cell.copy') }}
          </el-button>
        </div>
        <pre class="sql-card-body">{{ cellCardText }}</pre>
      </div>
    </el-popover>
    <SnippetSettings v-model="snippetSettingsVisible" :kind="tab.kind" />

    <!-- 单元格复制右键菜单（公共 ContextMenu 渲染） -->
    <ContextMenu
      v-model:visible="cellMenu.visible"
      :x="cellMenu.x"
      :y="cellMenu.y"
      :items="cellMenuItems"
      @select="onCellMenuSelect"
    />

    <!-- SQL INSERT 导出选项 -->
    <el-dialog
      v-model="sqlExportVisible"
      :title="t('result.sqlExportTitle')"
      width="480px"
      append-to-body
    >
      <el-form label-width="150px" size="small">
        <el-form-item :label="t('result.unquoteIdent')">
          <el-checkbox v-model="sqlOpts.unquoteIdent" />
        </el-form-item>
        <el-form-item :label="t('result.identCase')">
          <el-radio-group v-model="sqlOpts.identCase">
            <el-radio-button value="original">{{ t('result.identOriginal') }}</el-radio-button>
            <el-radio-button value="upper">{{ t('result.identUpper') }}</el-radio-button>
            <el-radio-button value="lower">{{ t('result.identLower') }}</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item :label="t('result.dateConvert')">
          <el-checkbox v-model="sqlOpts.dateConvert" />
        </el-form-item>
        <el-form-item label="UTF-8 BOM">
          <el-checkbox v-model="sqlOpts.bom" />
        </el-form-item>
        <el-form-item :label="t('result.batchSize')">
          <el-input-number
            v-model="sqlOpts.batchSize"
            :min="1"
            :max="1000"
            controls-position="right"
            style="width: 130px"
          />
          <span class="batch-hint">{{ t('result.batchHint') }}</span>
        </el-form-item>
        <el-form-item :label="t('settings.expValueRules')">
          <ValueRuleEditor v-model="exportValueRules" />
        </el-form-item>
        <el-form-item :label="t('result.exportCols')">
          <div class="export-cols">
            <el-checkbox
              :model-value="exportColCount === columns.length"
              :indeterminate="exportColCount > 0 && exportColCount < columns.length"
              @update:model-value="setAllExportCols"
            >
              {{ t('result.selectAll') }}
            </el-checkbox>
            <div class="export-col-list">
              <el-checkbox
                v-for="c in columns"
                :key="c.prop"
                v-model="chosenCols[c.prop]"
              >
                {{ c.name }}
              </el-checkbox>
            </div>
          </div>
        </el-form-item>
        <el-form-item :label="t('result.fileName')">
          <el-input v-model="exportFileName" />
        </el-form-item>
        <el-form-item :label="t('result.saveDir')">
          <div class="export-dir">
            <span
              class="dir-text"
              :title="chosenExportPath || wsStore.current?.exportDir || ''"
            >
              {{ chosenExportPath || wsStore.current?.exportDir || t('result.noExportDir') }}
            </span>
            <el-button size="small" @click="browseExportPath">{{ t('common.browse') }}</el-button>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button size="small" @click="sqlExportVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button size="small" type="primary" @click="confirmSqlExport">
          {{ t('result.exportRows', { n: exportRowsCount }) }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 编辑预览（批准后执行，事务提交） -->
    <el-dialog v-model="previewVisible" :title="t('edit.previewTitle')" width="660px" append-to-body>
      <el-alert
        v-if="editHighRisk"
        type="warning"
        :closable="false"
        show-icon
        :title="t('edit.highRiskAlert')"
        style="margin-bottom: 8px"
      />
      <pre class="dml-preview">{{ previewSql.join('\n') }}</pre>
      <div v-if="needTypeConfirm" class="type-confirm">
        <div class="tc-hint">{{ t('edit.typeConfirmHint', { table: editTable }) }}</div>
        <el-input
          v-model="typeConfirmText"
          size="small"
          :placeholder="editTable"
        />
      </div>
      <template #footer>
        <el-button size="small" :icon="CopyDocument" @click="copyDml">
          {{ t('common.copy') }}
        </el-button>
        <el-button size="small" @click="previewVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button
          size="small"
          type="primary"
          :loading="editPending"
          :disabled="needTypeConfirm && typeConfirmText.trim().toLowerCase() !== editTable.toLowerCase()"
          @click="executeEdits"
        >
          {{ t('edit.execute', { n: previewSql.length }) }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 加载 SQL 后选择连接 -->
    <el-dialog
      v-model="sqlConnPickerVisible"
      :title="t('script.pickConnTitle')"
      width="420px"
      append-to-body
    >
      <p class="load-sql-hint">{{ t('script.pickConnHint') }}</p>
      <el-select v-model="pickedConnId" style="width: 100%">
        <el-option
          v-for="c in connStore.saved"
          :key="c.id"
          :value="c.id"
          :label="`${c.name} · ${t('dbKind.' + c.kind)}`"
        />
      </el-select>
      <template #footer>
        <el-button size="small" @click="sqlConnPickerVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button size="small" type="primary" @click="confirmLoadSql">
          {{ t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onActivated, onBeforeUnmount, onDeactivated, onMounted, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Sortable from 'sortablejs'
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  Brush,
  CopyDocument,
  DataAnalysis,
  EditPen,
  FolderOpened,
  MagicStick,
  TrendCharts,
  ScaleToOriginal,
  Search,
  Setting,
} from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import { useConnectionStore } from '../stores/connections'
import { useLogStore } from '../stores/logs'
import { usePreferencesStore } from '../stores/preferences'
import { useTabsStore, looksLikeConnError, type QueryTab } from '../stores/tabs'
import { notifyFileSaved } from '../utils/notify'
import { cellText } from '../workers/filterCore'
import { useSnippetStore } from '../stores/snippets'
import SnippetSettings from './SnippetSettings.vue'
import HistoryPanel from './HistoryPanel.vue'
import EmptyState from './EmptyState.vue'
import ContextMenu, { type CtxMenuItem } from './ContextMenu.vue'
import ColVisPopover from './ColVisPopover.vue'
import ColValueFilter from './ColValueFilter.vue'
import SqlEditor from './SqlEditor.vue'
import CellDisplay from './cell-viewers/CellDisplay.vue'
import type { CellFormat } from './cell-viewers/CellDisplay.vue'
import CellDetailDrawer from './cell-viewers/CellDetailDrawer.vue'
import type { CellDetail } from './cell-viewers/CellDetailDrawer.vue'
import RecordView from './cell-viewers/RecordView.vue'
import PlanTree from './plan/PlanTree.vue'
import ValueRuleEditor from './ValueRuleEditor.vue'
import { formatSql } from '../utils/sqlFormat'
import { dispatchCommand, onAppCommand } from '../utils/commands'
import { cellToText, copyText } from '../utils/clipboard'
import { startWindowDrag } from '../utils/drag'
import { dbExplain, dbRegistry, exportResultRows, dbExecute, dbListIndexes, dbSetAutocommit, dbCommit, dbRollback, dbBuildDml, saveTextFile, readTextFile } from '../api/db'
import type { RowsExportFormat, SqlExportOptions, DmlRequest } from '../api/db'
import { useWorkspaceStore } from '../stores/workspaces'
import { useResultPipeline } from '../composables/useResultPipeline'
import { useEditUndo, type EditRow } from '../composables/useEditUndo'
import { ElMessage } from 'element-plus'
import type { CellValue, PlanNode, SemanticType } from '../types/db'

const { t } = useI18n()

const props = defineProps<{ tab: QueryTab }>()

const connStore = useConnectionStore()
const logStore = useLogStore()
const prefs = usePreferencesStore()
const tabsStore = useTabsStore()
const snippetStore = useSnippetStore()
const wsStore = useWorkspaceStore()

/** 当前连接是否在线（用于快速重连入口） */
const connected = computed(() => connStore.isConnected(props.tab.connectionId))

async function doReconnect(): Promise<void> {
  const ok = await tabsStore.reconnect(props.tab)
  if (ok) {
    ElMessage.success(t('conn.reconnected'))
    databases.value = await connStore
      .listDatabases(props.tab.connectionId)
      .catch(() => [])
  } else {
    ElMessage.error(t('conn.reconnectFailed'))
  }
}

/** 默认 SQL 文件名：{页签标题}-{时间戳}.sql */
function defaultSqlFileName(): string {
  const base = (props.tab.title || props.tab.table || 'query').replace(/[^\w\u4e00-\u9fa5.-]+/g, '_')
  const d = new Date()
  const p = (n: number): string => String(n).padStart(2, '0')
  const ts = `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`
  return `${base}-${ts}.sql`
}

/** 保存 SQL 至文件（文件名可在对话框中修改） */
async function saveSqlFile(): Promise<void> {
  const ws = wsStore.current
  const dir = (ws?.exportDir || ws?.dir || '').replace(/[\\/]+$/, '')
  const name = defaultSqlFileName()
  const path = await saveDialog({
    title: t('script.save'),
    defaultPath: dir ? `${dir}/${name}` : name,
    filters: [
      { name: 'SQL', extensions: ['sql'] },
      { name: 'All Files', extensions: ['*'] },
    ],
  })
  if (!path || Array.isArray(path)) return
  try {
    await saveTextFile(path, props.tab.sql)
    ElMessage.success(t('script.saved'))
  } catch (e) {
    ElMessage.error(String(e))
  }
}

/** 从文件加载 SQL（加载后提示选择连接） */
const sqlConnPickerVisible = ref(false)
const loadedSqlText = ref('')
const pickedConnId = ref('')

async function openSqlFile(): Promise<void> {
  const sel = await openDialog({
    multiple: false,
    filters: [
      { name: 'SQL', extensions: ['sql', 'txt'] },
      { name: 'All Files', extensions: ['*'] },
    ],
  })
  if (!sel || Array.isArray(sel)) return
  try {
    loadedSqlText.value = await readTextFile(sel)
    pickedConnId.value = props.tab.connectionId
    sqlConnPickerVisible.value = true
  } catch (e) {
    ElMessage.error(String(e))
  }
}

function confirmLoadSql(): void {
  const cfg = connStore.saved.find((c) => c.id === pickedConnId.value)
  if (!cfg) {
    ElMessage.warning(t('script.pickConn'))
    return
  }
  props.tab.connectionId = cfg.id
  props.tab.connectionName = cfg.name
  props.tab.kind = cfg.kind
  props.tab.database = ''
  props.tab.result = null
  props.tab.error = ''
  props.tab.errorKind = undefined
  props.tab.sql = loadedSqlText.value
  sqlConnPickerVisible.value = false
  ElMessage.success(t('script.loaded'))
}
const databases = ref<string[]>([])
const editorRef = ref<InstanceType<typeof SqlEditor>>()
const snippetSettingsVisible = ref(false)

/** 结果视图模式：表格 / 记录 / 执行计划 */
const viewMode = ref<'grid' | 'record' | 'plan'>('grid')

// ---------- 多结果集 ----------
const resultCount = computed(() => props.tab.multiResults?.length ?? 0)
const resultOptions = computed(() =>
  (props.tab.multiResults ?? []).map((r, i) => ({
    label: `${t('result.nth', { n: i + 1 })} (${r.rows.length})`,
    value: i,
  })),
)

function switchResult(index: number): void {
  props.tab.activeResultIndex = index
  props.tab.result = props.tab.multiResults?.[index] ?? props.tab.result
}

// ---------- 虚拟滚动大结果模式（>600 行且非编辑态） ----------
const VIRTUAL_THRESHOLD = 600
const useVirtualGrid = computed(
  () => !editMode.value && (props.tab.result?.rows.length ?? 0) > VIRTUAL_THRESHOLD,
)
const virtualColumns = computed(() =>
  columns.value.map((c) => ({
    name: c.name,
    semantic: c.semantic,
    srcIndex: propIndex.value.get(c.prop) ?? 0,
  })),
)
/** 零拷贝：直接引用过滤后的原始矩阵行（vgrid 经 srcIndex 取值） */
const virtualRows = computed(() => filteredIdx.value.map((i) => matrix.value[i]))

const ResultVirtualGrid = defineAsyncComponent(() => import('./ResultVirtualGrid.vue'))

function onVirtualCellMenu(p: {
  rowIndex: number
  colIndex: number
  value: unknown
  x: number
  y: number
}): void {
  const text = cellToText(p.value as CellValue)
  void copyText(text)
  ElMessage.success(t('cell.copied'))
}

/** 虚拟模式双击整行复制（TSV） */
function onVirtualRowCopy(rowIndex: number): void {
  const cells = virtualRows.value[rowIndex]
  if (!cells) return
  void copyText(cells.map((v) => cellToText(v)).join('\t'))
  ElMessage.success(t('result.rowCopied'))
}
/** 编辑器区域高度（可上下拖拽，localStorage 持久化） */
const EDITOR_H_KEY = 'personaldb.editorHeight'
const editorWrapHeight = ref(
  Math.min(700, Math.max(80, Number(localStorage.getItem(EDITOR_H_KEY)) || 220)),
)

function startEditorDrag(e: MouseEvent): void {
  e.preventDefault()
  const startY = e.clientY
  const startH = editorWrapHeight.value
  startWindowDrag(
    (ev) => {
      editorWrapHeight.value = Math.min(700, Math.max(80, startH + (ev.clientY - startY)))
    },
    () => localStorage.setItem(EDITOR_H_KEY, String(editorWrapHeight.value)),
  )
}

/** 单元格复制：选中 + 右键菜单 + Ctrl+C */
const cellMenu = ref({ visible: false, x: 0, y: 0 })
const selectedCell = ref<{ row: Record<string, CellValue>; prop: string } | null>(null)

function onCellClick(row: Record<string, CellValue>, column: { property?: string }): void {
  if (column.property) selectedCell.value = { row, prop: column.property }
}

function onCellContextMenu(
  row: Record<string, CellValue>,
  column: { property?: string },
  _cell: unknown,
  event: MouseEvent,
): void {
  if (!column.property) return
  selectedCell.value = { row, prop: column.property }
  menuGridIdx.value = rowIndexOf(row)
  menuRowIdx.value = editMode.value ? editData.value.indexOf(row) : -1
  cellMenu.value = { visible: true, x: event.clientX, y: event.clientY }
}

/** 单元格右键菜单项（公共 ContextMenu 渲染；按选区/视图/编辑态动态成组） */
const cellMenuItems = computed<CtxMenuItem[]>(() => {
  const items: CtxMenuItem[] = []
  if (selectedRows.value.length) {
    items.push({ key: 'copySelection', label: `${t('record.copySelection')}（${selectedRows.value.length}）`, icon: 'act_copy' })
  }
  if (viewMode.value === 'grid' && !editMode.value) {
    items.push({ key: 'openInRecord', label: t('record.openInRecord'), icon: 'obj_column' })
  }
  if (items.length) items.push({ key: 'sep1', label: '', sep: true })
  if (editMode.value) {
    items.push(
      { key: 'addRow', label: t('edit.addRow'), icon: 'act_plus' },
      { key: 'copyRowEdit', label: t('edit.copyRow'), icon: 'act_copy' },
      { key: 'markDelete', label: t('edit.markDelete'), icon: 'act_delete', danger: true },
      { key: 'undoRow', label: t('edit.undoRow'), icon: 'act_undo' },
      { key: 'sep2', label: '', sep: true },
    )
  }
  items.push(
    { key: 'copyCell', label: t('record.copyCell'), icon: 'act_copy' },
    { key: 'copyRow', label: t('record.copyRow'), icon: 'act_copy' },
    { key: 'copyColumn', label: t('record.copyColumn'), icon: 'act_copy' },
  )
  return items
})

function onCellMenuSelect(key: string): void {
  switch (key) {
    case 'copySelection':
      void copySelectedRows()
      break
    case 'openInRecord':
      viewInRecord()
      break
    case 'addRow':
      addEditRow()
      break
    case 'copyRowEdit':
      duplicateEditRow(menuRowIdx.value)
      break
    case 'markDelete':
      toggleDeleteRow(menuRowIdx.value)
      break
    case 'undoRow':
      undoEditRow(menuRowIdx.value)
      break
    case 'copyCell':
      void copySelectedCell()
      break
    case 'copyRow':
      void copyRow()
      break
    case 'copyColumn':
      void copyColumn()
      break
  }
}

/** 行在 resultRows 中的下标（先按引用，再按值回退） */
function rowIndexOf(row: Record<string, CellValue>): number {
  const byRef = resultRows.value.indexOf(row)
  if (byRef >= 0) return byRef
  return resultRows.value.findIndex((r) => {
    const a = Object.keys(r)
    const b = Object.keys(row)
    if (a.length !== b.length) return false
    return a.every((k) => r[k] === (row as Record<string, CellValue>)[k])
  })
}

/** 编辑态：行样式（新增/删除/已改） */
function editRowClass({ rowIndex }: { rowIndex: number }): string {
  if (!editMode.value) return ''
  const r = editRows.value[rowIndex]
  if (!r) return ''
  if (r.deleted) return 'edit-deleted'
  if (r.isNew) return 'edit-new'
  return Object.keys(r.values).some((k) => r.values[k] !== r.orig[k]) ? 'edit-dirty' : ''
}

function closeCellMenu(): void {
  cellMenu.value.visible = false
}

/** 记录视图初始定位行（0 基） */
const recordStart = ref(0)
/** 右键时的行号（打开菜单时记录，避免点击瞬间引用失配） */
const menuGridIdx = ref(-1)

/** 右键「在记录视图中查看」 */
function viewInRecord(): void {
  recordStart.value = menuGridIdx.value >= 0 ? menuGridIdx.value : 0
  viewMode.value = 'record'
  closeCellMenu()
}

async function copySelectedCell(): Promise<void> {
  const sel = selectedCell.value
  if (!sel) return
  await copyText(cellToText(sel.row[sel.prop]))
  closeCellMenu()
}

async function copyRow(): Promise<void> {
  const sel = selectedCell.value
  if (!sel) return
  const line = columns.value.map((c) => cellToText(sel.row[c.prop])).join('\t')
  await copyText(line)
  closeCellMenu()
}

async function copyColumn(): Promise<void> {
  const sel = selectedCell.value
  if (!sel) return
  const text = resultRows.value.map((r) => cellToText(r[sel.prop])).join('\n')
  await copyText(text)
  closeCellMenu()
}

function onGlobalKeydown(e: KeyboardEvent): void {
  // Ctrl+S：保存 SQL 脚本（编辑器/普通焦点均可，单元格输入中除外）
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 's') {
    const el = document.activeElement as HTMLElement | null
    if (el && el.closest('input, textarea')) return
    e.preventDefault()
    void saveSqlFile()
    return
  }
  // Ctrl+A：结果表全选（非编辑态且有选择列）
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'a') {
    const el = document.activeElement as HTMLElement | null
    if (el && el.closest('input, textarea, [contenteditable="true"], .cm-editor')) return
    if (viewMode.value === 'grid' && !editMode.value && tableRef.value) {
      e.preventDefault()
      tableRef.value?.toggleAllSelection?.()
      return
    }
  }
  // Ctrl+F 聚焦结果查找框
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'f') {
    const el = document.activeElement as HTMLElement | null
    if (el && el.closest('.cm-editor')) return
    e.preventDefault()
    searchInputRef.value?.focus()
    return
  }
  if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'c') return
  const el = document.activeElement as HTMLElement | null
  if (el && el.closest('input, textarea, [contenteditable="true"], .cm-editor')) return
  if (window.getSelection()?.toString()) return
  if (selectedRows.value.length > 0) {
    e.preventDefault()
    void copySelectedRows()
    return
  }
  if (!selectedCell.value) return
  e.preventDefault()
  void copySelectedCell()
}

/** 多行选择（勾选 + Shift 范围选择） */
const selectedRows = ref<Array<Record<string, CellValue>>>([])
let lastClickedIdx = -1

function onSelectionChange(rows: Array<Record<string, CellValue>>): void {
  selectedRows.value = rows
}

function onRowClick(
  row: Record<string, CellValue>,
  _column: unknown,
  event: MouseEvent,
): void {
  const idx = resultRows.value.indexOf(row)
  if (idx < 0) return
  const tbl = tableRef.value as
    | { toggleRowSelection?: (r: unknown, selected?: boolean) => void }
    | undefined
  if (!tbl?.toggleRowSelection) return
  if (event.shiftKey && lastClickedIdx >= 0) {
    const lo = Math.min(lastClickedIdx, idx)
    const hi = Math.max(lastClickedIdx, idx)
    for (let i = lo; i <= hi; i++) {
      tbl.toggleRowSelection(resultRows.value[i], true)
    }
  } else {
    tbl.toggleRowSelection(row)
  }
  lastClickedIdx = idx
}

async function copySelectedRows(): Promise<void> {
  if (selectedRows.value.length === 0) return
  const cols = columns.value
  const text = selectedRows.value
    .map((r) => cols.map((c) => cellToText(r[c.prop])).join('\t'))
    .join('\n')
  await copyText(text)
  closeCellMenu()
}

/** 选中行导出（无选中 = 全部行；仅可见列） */
async function onExportRows(command: string): Promise<void> {
  const format = command as RowsExportFormat
  const rows = selectedRows.value.length > 0 ? selectedRows.value : resultRows.value
  if (rows.length === 0) return
  const cols = columns.value
  if (cols.length === 0) return

  // SQL INSERT：先弹选项对话框（默认取首选项；可筛列/改合并行数）
  if (format === 'sql') {
    exportRowsCount.value = rows.length
    exportFileName.value = defaultExportName('sql')
    chosenExportPath.value = ''
    sqlOpts.unquoteIdent = prefs.exportPrefs.unquoteIdent
    sqlOpts.identCase = prefs.exportPrefs.identCase
    sqlOpts.dateConvert = prefs.exportPrefs.dateConvert
    sqlOpts.bom = prefs.exportPrefs.bom
    sqlOpts.batchSize = prefs.exportPrefs.insertBatchSize
    exportValueRules.value = prefs.exportPrefs.valueRules.map((r) => ({ ...r }))
    chosenCols.value = Object.fromEntries(columns.value.map((c) => [c.prop, true]))
    sqlExportVisible.value = true
    return
  }

  const ext = format === 'excel' ? 'xlsx' : format
  const path = await saveDialog({
    title: t('result.exportSelected'),
    defaultPath: defaultExportPath(ext),
    filters: [
      { name: ext.toUpperCase(), extensions: [ext] },
      { name: 'All Files', extensions: ['*'] },
    ],
  })
  if (!path || Array.isArray(path)) return
  await doExportRows(format, path, rows, cols, undefined)
}

async function doExportRows(
  format: RowsExportFormat,
  path: string,
  rows: Array<Record<string, CellValue>>,
  cols: Array<{ name: string; prop: string; semantic: SemanticType }>,
  sqlOptions: SqlExportOptions | undefined,
): Promise<void> {
  try {
    const summary = await exportResultRows({
      kind: props.tab.kind,
      table: props.tab.table ?? 'query_result',
      schema: null,
      columns: cols.map((c) => ({ name: c.name, semantic: c.semantic })),
      rows: rows.map((r) => cols.map((c) => r[c.prop] ?? null)),
      format,
      path,
      sqlOptions,
    })
    ElMessage.success(t('result.exported', { n: summary.rows }))
    notifyFileSaved(path, t('result.exported', { n: summary.rows }))
  } catch (e) {
    ElMessage.error(String(e))
  }
}

/** 默认文件名：export_{table}_{yyyyMMddHHmmss}.{ext} */
function defaultExportName(ext: string): string {
  const tbl = (props.tab.table ?? 'result').replace(/[^\w.-]+/g, '_')
  const d = new Date()
  const p = (n: number): string => String(n).padStart(2, '0')
  const ts = `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`
  return `export_${tbl}_${ts}.${ext}`
}

/** 默认保存位置：当前工作区导出目录，未设则取工作区存储目录（均无则文件名） */
function defaultExportPath(ext: string): string {
  const ws = wsStore.current
  const dir = (ws?.exportDir || ws?.dir || '').replace(/[\\/]+$/, '')
  const name = defaultExportName(ext)
  return dir ? `${dir}/${name}` : name
}

/** SQL INSERT 导出选项对话框 */
const sqlExportVisible = ref(false)
const exportRowsCount = ref(0)
const exportFileName = ref('')
const chosenExportPath = ref('')
const sqlOpts = reactive({
  unquoteIdent: false,
  identCase: 'original' as 'original' | 'upper' | 'lower',
  dateConvert: true,
  bom: false,
  batchSize: 10,
})
/** 导出列筛选（默认全部展示列） */
const chosenCols = ref<Record<string, boolean>>({})
const exportColCount = computed(() => Object.values(chosenCols.value).filter(Boolean).length)
/** 本次导出的值转换规则（默认取首选项） */
const exportValueRules = ref<Array<{ semantic: string; template: string }>>([])

function setAllExportCols(v: boolean | string | number | undefined): void {
  if (v) {
    chosenCols.value = Object.fromEntries(columns.value.map((c) => [c.prop, true]))
  }
}

function browseExportPath(): void {
  void (async () => {
    const path = await saveDialog({
      title: t('result.exportSelected'),
      defaultPath: defaultExportPath('sql'),
      filters: [
        { name: 'SQL', extensions: ['sql'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (!path || Array.isArray(path)) return
    chosenExportPath.value = path
  })()
}

async function confirmSqlExport(): Promise<void> {
  const rows = selectedRows.value.length > 0 ? selectedRows.value : resultRows.value
  const cols = columns.value.filter((c) => chosenCols.value[c.prop])
  if (rows.length === 0 || cols.length === 0) {
    ElMessage.warning(t('result.noExportCols'))
    return
  }
  let path = chosenExportPath.value
  if (!path) {
    const dir = wsStore.current?.exportDir?.replace(/[\\/]+$/, '')
    if (!dir) {
      // 无默认目录时回退到系统保存对话框
      const p = await saveDialog({
        title: t('result.exportSelected'),
        defaultPath: exportFileName.value,
        filters: [{ name: 'SQL', extensions: ['sql'] }],
      })
      if (!p || Array.isArray(p)) return
      path = p
    } else {
      path = `${dir}/${exportFileName.value}`
    }
  }
  sqlExportVisible.value = false
  await doExportRows('sql', path, rows, cols, { ...sqlOpts, valueRules: exportValueRules.value })
}

/* ================= 结果集编辑（增删改：预览 + 事务提交） ================= */

const editMode = ref(false)
const editRows = ref<EditRow[]>([])
const editingCell = ref<{ rid: number; prop: string } | null>(null)
const editValue = ref('')
const editPending = ref(false)
const editTable = ref('')
const editWhereIndices = ref<number[]>([])
const editHighRisk = ref(false)
const previewVisible = ref(false)
/** 高危（全列匹配）确认：需输入表名才能执行 */
const typeConfirmText = ref('')
const needTypeConfirm = computed(() => editHighRisk.value)
const previewSql = ref<string[]>([])

const editData = computed(() => (editMode.value ? editRows.value.map((r) => r.values) : []))

const editCounts = computed(() => {
  let ins = 0
  let upd = 0
  let del = 0
  for (const r of editRows.value) {
    if (r.deleted) del++
    else if (r.isNew) ins++
    else if (Object.keys(r.values).some((k) => r.values[k] !== r.orig[k])) upd++
  }
  return { ins, upd, del, total: ins + upd + del }
})

/** 解析单表 SELECT 的来源表；多表/连接返回 null */
function parseSingleTable(sql: string): string | null {
  const m = /\bfrom\s+([`"[\].\w$]+)/i.exec(sql)
  if (!m) return null
  const tail = sql.slice(m.index + m[0].length)
  const stop = tail.search(/\b(where|group\s+by|order\s+by|limit|having|union)\b/i)
  const seg = (stop >= 0 ? tail.slice(0, stop) : tail).trim()
  if (/,/.test(seg) || /\bjoin\b/i.test(seg)) return null
  return m[1].replace(/[`"[\]]/g, '')
}

const vFocus = {
  mounted: (el: HTMLElement): void => {
    const input = el.querySelector('input') as HTMLInputElement | null
    ;(input ?? el).focus()
  },
}

function exitEdit(): void {
  editMode.value = false
  editRows.value = []
  editingCell.value = null
}

async function toggleEdit(): Promise<void> {
  if (editMode.value) {
    exitEdit()
    return
  }
  const session = connStore.sessionOf(props.tab.connectionId)
  if (!session || !props.tab.result) return
  const table = props.tab.table ?? parseSingleTable(props.tab.sql)
  if (!table) {
    ElMessage.warning(t('edit.onlySingleTable'))
    return
  }
  let pk: string[] = []
  try {
    const idx = await dbListIndexes(session.sessionId, props.tab.database || null, table)
    pk = idx.find((i) => i.primary)?.columns ?? []
  } catch {
    pk = []
  }
  const cols = columns.value
  const baseName = (n: string): string => n.replace(/ \(\d+\)$/, '').toLowerCase()
  const idxOf = (name: string): number => cols.findIndex((c) => baseName(c.name) === name.toLowerCase())
  let where = pk.map(idxOf).filter((i) => i >= 0)
  let risk = false
  if (where.length === 0) {
    const rowidIdx = cols.findIndex((c) => baseName(c.name) === 'rowid')
    if (rowidIdx >= 0) {
      where = [rowidIdx]
    } else {
      where = cols.map((_, i) => i)
      risk = true
    }
  }
  editTable.value = table
  editWhereIndices.value = where
  editHighRisk.value = risk
  editRows.value = resultRows.value.map((r) => ({
    orig: { ...r },
    values: { ...r },
    isNew: false,
    deleted: false,
  }))
  editUndoStack.value = []
  editRedoStack.value = []
  editMode.value = true
}

const menuRowIdx = ref(-1)

function beginEdit(rid: number, prop: string): void {
  if (editRows.value[rid]?.deleted) return
  editingCell.value = { rid, prop }
  editValue.value = String(editRows.value[rid].values[prop] ?? '')
}

function coerceValue(prop: string, s: string): CellValue {
  const col = columns.value.find((c) => c.prop === prop)
  if (col && (col.semantic === 'number' || col.semantic === 'decimal')) {
    const n = Number(s)
    return s.trim() !== '' && Number.isFinite(n) ? n : s
  }
  if (col && col.semantic === 'bool') {
    if (/^(true|1|y|yes)$/i.test(s)) return true
    if (/^(false|0|n|no)$/i.test(s)) return false
  }
  return s
}

function commitEdit(): void {
  if (!editingCell.value) return
  const { rid, prop } = editingCell.value
  const row = editRows.value[rid]
  if (row && String(row.values[prop] ?? '') !== editValue.value) pushEditUndo()
  if (row) row.values[prop] = coerceValue(prop, editValue.value)
  editingCell.value = null
}

function cancelEdit(): void {
  editingCell.value = null
}

function addEditRow(): void {
  pushEditUndo()
  editRows.value.push({
    orig: {},
    values: Object.fromEntries(columns.value.map((c) => [c.prop, null])),
    isNew: true,
    deleted: false,
  })
}

function duplicateEditRow(idx: number): void {
  const src = editRows.value[idx]
  if (!src) return
  pushEditUndo()
  editRows.value.push({ orig: {}, values: { ...src.values }, isNew: true, deleted: false })
}

function toggleDeleteRow(idx: number): void {
  const r = editRows.value[idx]
  if (!r) return
  pushEditUndo()
  if (r.isNew) {
    editRows.value.splice(idx, 1)
    return
  }
  r.deleted = !r.deleted
}

function undoEditRow(idx: number): void {
  const r = editRows.value[idx]
  if (!r) return
  pushEditUndo()
  r.values = { ...r.orig }
  r.deleted = false
}

// ---------- 编辑撤销/重做（Ctrl+Z / Ctrl+Y，栈深 50） ----------
const {
  editUndoStack,
  editRedoStack,
  pushEditUndo,
  undoGridEdit,
  redoGridEdit,
  onEditKeydown,
} = useEditUndo(editRows, () => editMode.value && !editingCell.value)

function buildDmlRequest(): DmlRequest {
  const cols = columns.value
  const inserts = editRows.value
    .filter((r) => r.isNew && !r.deleted)
    .map((r) => cols.map((c) => r.values[c.prop] ?? null))
  const updates = editRows.value
    .filter((r) => !r.isNew && !r.deleted && Object.keys(r.values).some((k) => r.values[k] !== r.orig[k]))
    .map((r) => ({
      old: cols.map((c) => r.orig[c.prop] ?? null),
      new: cols.map((c) => r.values[c.prop] ?? null),
    }))
  const deletes = editRows.value
    .filter((r) => r.deleted)
    .map((r) => cols.map((c) => r.orig[c.prop] ?? null))
  return {
    kind: props.tab.kind,
    schema: null,
    table: editTable.value,
    columns: cols.map((c) => ({ name: c.name, semantic: c.semantic })),
    whereIndices: editWhereIndices.value,
    inserts,
    updates,
    deletes,
  }
}

async function openPreview(): Promise<void> {
  if (editCounts.value.total === 0) return
  try {
    previewSql.value = await dbBuildDml(buildDmlRequest())
    if (previewSql.value.length === 0) {
      ElMessage.warning(t('edit.nothing'))
      return
    }
    typeConfirmText.value = ''
    previewVisible.value = true
  } catch (e) {
    ElMessage.error(String(e))
  }
}

/** 复制预览的 DML 语句 */
async function copyDml(): Promise<void> {
  if (previewSql.value.length === 0) return
  await copyText(previewSql.value.join('\n'))
}

async function executeEdits(): Promise<void> {
  const session = connStore.sessionOf(props.tab.connectionId)
  if (!session) return
  editPending.value = true
  try {
    await dbSetAutocommit(session.sessionId, false)
    try {
      for (const sql of previewSql.value) {
        await dbExecute(session.sessionId, props.tab.database || null, sql, 1)
      }
      await dbCommit(session.sessionId)
      ElMessage.success(t('edit.applied', { n: previewSql.value.length }))
    } catch (e) {
      await dbRollback(session.sessionId).catch(() => {})
      throw e
    } finally {
      await dbSetAutocommit(session.sessionId, true).catch(() => {})
    }
    previewVisible.value = false
    exitEdit()
    await tabsStore.runTab(props.tab)
  } catch (e) {
    logStore.push({
      level: 'error',
      kind: 'sql',
      title: t('edit.appliedTitle'),
      detail: String(e),
      conn: props.tab.connectionName,
      db: props.tab.database,
    })
    ElMessage.error(String(e))
  } finally {
    editPending.value = false
  }
}

const viewOptions = computed(() => {
  const base: Array<{ label: string; value: string }> = [
    { label: t('query.viewGrid'), value: 'grid' },
    { label: t('query.viewRecord'), value: 'record' },
  ]
  if (planData.value) base.push({ label: t('query.viewPlan'), value: 'plan' })
  return base
})

/** 执行计划 */
const explainCap = ref(false)
const explaining = ref(false)
const planData = ref<PlanNode | null>(null)

async function runExplain(): Promise<void> {
  const session = connStore.sessionOf(props.tab.connectionId)
  if (!session || !props.tab.sql.trim()) return
  explaining.value = true
  try {
    planData.value = await dbExplain(
      session.sessionId,
      props.tab.database || null,
      props.tab.sql,
    )
    viewMode.value = 'plan'
  } catch (e) {
    props.tab.error = String(e)
    props.tab.errorKind = undefined
  } finally {
    explaining.value = false
  }
}

/** 按方言格式化（外部文本变化经 v-model 同步到编辑器） */
function doFormat(): void {
  if (!props.tab.sql.trim()) return
      props.tab.sql = formatSql(props.tab.kind, props.tab.sql, prefs.formatPrefs)
}

/** 压缩 SQL（去注释/多余空白，字面值保护） */
function doCompress(): void {
  editorRef.value?.compress()
}
const detailVisible = ref(false)
const detailState = ref<CellDetail | null>(null)

function onDetail(payload: { col: { name: string; typeLabel?: string; semantic?: SemanticType }; value: CellValue }): void {
  if (payload.value === null || payload.value === undefined) return
  detailState.value = {
    name: payload.col.name,
    typeLabel: payload.col.typeLabel ?? '',
    semantic: payload.col.semantic ?? 'unknown',
    value: payload.value,
  }
  detailVisible.value = true
}

const snippetMap = computed(() => snippetStore.mergedFor(props.tab.kind))

let offCommand: (() => void) | null = null

/** 菜单栏命令（仅响应指向当前 Tab 的命令） */
function handleAppCommand(payload: { cmd: string; tabId?: string }): void {
  if (payload.tabId && payload.tabId !== props.tab.id) return
  switch (payload.cmd) {
    case 'format':
      doFormat()
      break
    case 'compress':
      doCompress()
      break
    case 'upper':
      editorRef.value?.transformCase('upper')
      break
    case 'lower':
      editorRef.value?.transformCase('lower')
      break
    case 'view-grid':
      viewMode.value = 'grid'
      break
    case 'view-record':
      if (props.tab.result && props.tab.result.columns.length > 0) viewMode.value = 'record'
      break
    case 'view-plan':
      if (planData.value) viewMode.value = 'plan'
      break
    case 'snippets':
      snippetSettingsVisible.value = true
      break
  }
}

/** 全局键盘/粘贴监听（KeepAlive 下随激活状态启停：隐藏的页签不响应用户按键） */
function bindPaneEvents(): void {
  window.addEventListener('keydown', onGlobalKeydown)
  window.addEventListener('keydown', onEditKeydown)
  window.addEventListener('paste', onGridPaste)
}

function unbindPaneEvents(): void {
  window.removeEventListener('keydown', onGlobalKeydown)
  window.removeEventListener('keydown', onEditKeydown)
  window.removeEventListener('paste', onGridPaste)
}

onMounted(async () => {
  offCommand = onAppCommand(handleAppCommand)
  bindPaneEvents()
  if (connStore.isConnected(props.tab.connectionId)) {
    databases.value = await connStore
      .listDatabases(props.tab.connectionId)
      .catch(() => [])
  }
  // 能力驱动：当前数据库是否支持执行计划
  try {
    const reg = await dbRegistry()
    explainCap.value = reg.find((k) => k.kind === props.tab.kind)?.explain ?? false
  } catch {
    explainCap.value = false
  }
})

onBeforeUnmount(() => {
  offCommand?.()
  sortable?.destroy()
  sortable = null
  boundScrollEl?.removeEventListener('scroll', onTableScroll)
  boundScrollEl = null
  unbindPaneEvents()
})

// KeepAlive 生命周期：onMounted 与 onActivated 首次会先后触发，addEventListener 按
// 同一函数引用去重，不会重复注册；onDeactivated 移除保证隐藏页签不抢全局按键/粘贴
onActivated(bindPaneEvents)
onDeactivated(unbindPaneEvents)

/** 执行：auto=有选中执行选中否则全部 / all=全部 / selection=选中 / line=当前行 */
async function run(mode: 'all' | 'selection' | 'line' | 'auto'): Promise<void> {
  if (mode === 'auto' || mode === 'selection') {
    const text = editorRef.value?.getSelection()
    if (text && text.trim()) {
      await tabsStore.runTab(props.tab, text)
      return
    }
    if (mode === 'selection') return
  } else if (mode === 'line') {
    const text = editorRef.value?.getCurrentLine()
    if (text && text.trim()) {
      await tabsStore.runTab(props.tab, text)
      return
    }
  }
  await tabsStore.runTab(props.tab)
}

/** 列级显示设置（格式 / 固定 / 筛选），未设置项按全局偏好 */
interface ColSetting {
  format?: CellFormat
  fixed?: 'left' | 'right'
  filter?: string
}

const colSettings = ref<Record<string, ColSetting>>({})
/** 列顺序（拖拽调整） */
const colOrder = ref<string[]>([])
/** 全局查找关键字（输入防抖 200ms 生效） */
const searchText = ref('')
const searchApplied = ref('')
let searchTimer: number | undefined
watch(searchText, (v) => {
  window.clearTimeout(searchTimer)
  searchTimer = window.setTimeout(() => {
    searchApplied.value = v
  }, 200)
})
const searchInputRef = ref()
const tableRef = ref()

const allCols = computed(() => {
  const result = props.tab.result
  if (!result) return []
  const seen = new Map<string, number>()
  const cols = result.columns.map((c) => {
    const count = seen.get(c.name) ?? 0
    seen.set(c.name, count + 1)
    const semantic = (c.semantic ?? 'unknown') as SemanticType
    const prop = `c${count}_${c.name}`
    return {
      name: count > 0 ? `${c.name} (${count + 1})` : c.name,
      prop,
      typeLabel: c.typeLabel,
      semantic,
      align: (semantic === 'number' || semantic === 'decimal' ? 'right' : 'left') as
        | 'left'
        | 'right',
      format: effectiveFormat(prop, semantic),
      fixed: colSettings.value[prop]?.fixed,
    }
  })
  return cols
})

/** 展示列 = 全部列 − 勾选隐藏列，再应用拖拽顺序 */
const columns = computed(() => {
  const cols = allCols.value.filter((c) => !hiddenProps.value[c.prop])
  if (colOrder.value.length > 0) {
    cols.sort((a, b) => {
      const ia = colOrder.value.indexOf(a.prop)
      const ib = colOrder.value.indexOf(b.prop)
      return (ia < 0 ? Number.MAX_SAFE_INTEGER : ia) - (ib < 0 ? Number.MAX_SAFE_INTEGER : ib)
    })
  }
  return cols
})

/** 勾选展示列：隐藏的列 prop 集合（已应用状态） */
const hiddenProps = ref<Record<string, boolean>>({})
/** 显示列弹层已隔离至 ColVisPopover 子组件（勾选过程零父渲染） */
function applyColVis(next: Record<string, boolean>): void {
  hiddenProps.value = { ...next }
}

/** 全量列（按当前 colOrder 排列，供显示列弹层展示/拖拽） */
const orderedAllCols = computed(() => {
  if (colOrder.value.length === 0) return allCols.value
  const map = new Map(allCols.value.map((c) => [c.prop, c]))
  const ordered = colOrder.value.map((p) => map.get(p)).filter(Boolean) as typeof allCols.value
  for (const c of allCols.value) {
    if (!ordered.some((o) => o.prop === c.prop)) ordered.push(c)
  }
  return ordered
})

function onColsReorder(order: string[]): void {
  colOrder.value = order
}

/** 列移到最前 / 最后（基于当前可见列全序） */
function moveColEdge(prop: string, edge: 'front' | 'back'): void {
  const base = allCols.value.map((c) => c.prop)
  const order = colOrder.value.length > 0 ? [...colOrder.value] : [...base]
  const idx = order.indexOf(prop)
  if (idx < 0) return
  order.splice(idx, 1)
  if (edge === 'front') order.unshift(prop)
  else order.push(prop)
  colOrder.value = order
}

/** 还原列设置：顺序 / 宽度 / 显隐 全部恢复默认 */
function resetCols(): void {
  colOrder.value = []
  colWidths.value = {}
  hiddenProps.value = {}
  try {
    const all = JSON.parse(localStorage.getItem('personaldb.colWidths') ?? '{}') as Record<string, Record<string, number>>
    delete all[widthKey.value]
    localStorage.setItem('personaldb.colWidths', JSON.stringify(all))
  } catch {
    /* ignore */
  }
}

/** 表头列拖拽排序（sortablejs） */
let sortable: Sortable | null = null

function initSortable(): void {
  const el = (tableRef.value?.$el as HTMLElement | undefined)?.querySelector(
    '.el-table__header-wrapper thead tr',
  )
  if (!el) return
  sortable?.destroy()
  // 存在固定列时禁用拖拽（固定列渲染于独立表头，拖拽会错位）
  if (columns.value.some((c) => c.fixed)) return
  sortable = Sortable.create(el as HTMLElement, {
    animation: 150,
    onEnd: ({ oldIndex, newIndex }) => {
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return
      const order = columns.value.map((c) => c.prop)
      const [moved] = order.splice(oldIndex, 1)
      order.splice(newIndex, 0, moved)
      colOrder.value = order
      void nextTick(initSortable)
    },
  })
}

watch(
  () => [columns.value.length, columns.value.some((c) => c.fixed)],
  () => void nextTick(initSortable),
)

/** 表格滚动触底：自动续拉下一页 */
let boundScrollEl: HTMLElement | null = null

function onTableScroll(e: Event): void {
  const el = e.target as HTMLElement
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 60) {
    void tabsStore.loadMore(props.tab)
  }
}

function bindTableScroll(): void {
  const el = (tableRef.value?.$el as HTMLElement | undefined)?.querySelector(
    '.el-scrollbar__wrap',
  )
  if (!el || el === boundScrollEl) return
  boundScrollEl?.removeEventListener('scroll', onTableScroll)
  boundScrollEl = el as HTMLElement
  boundScrollEl.addEventListener('scroll', onTableScroll, { passive: true })
}

watch(
  () => [props.tab.result?.rows.length, viewMode.value],
  () => void nextTick(bindTableScroll),
)

const hasColSetting = (prop: string): boolean => Boolean(colSettings.value[prop])

/** 列筛选：回车/失焦才提交（避免逐键触发整表重渲染） */
function commitColFilter(prop: string, v: string): void {
  setColOption(prop, 'filter', v || undefined)
}

function setColOption(prop: string, key: keyof ColSetting, value: unknown): void {
  const cur = colSettings.value[prop] ?? {}
  colSettings.value = {
    ...colSettings.value,
    [prop]: { ...cur, [key]: value === '' ? undefined : value },
  }
}

function effectiveFormat(prop: string, semantic: SemanticType): CellFormat {
  const override = colSettings.value[prop]?.format
  if (override) return override
  if (semantic === 'json' && prefs.result.formatJson) return 'pretty'
  if (isDateLike(semantic)) {
    const ds = prefs.dataFormat.dateStyle
    if (ds === 'local') return 'local'
    if (ds === 'iso') return 'iso'
  }
  if (
    prefs.dataFormat.thousand &&
    (semantic === 'number' || semantic === 'decimal')
  ) {
    return 'thousand'
  }
  return 'raw'
}

/** 行高（按偏好三档） */
const rowHeight = computed(
  () => ({ compact: '22px', normal: '28px', loose: '36px' })[prefs.result.rowHeight] ?? '28px',
)
const isCompact = computed(() => prefs.result.rowHeight === 'compact')

/** 已达 maxTotalRows 上限：truncated 但不再续拉 */
const reachLimit = computed(
  () =>
    Boolean(props.tab.result?.truncated) &&
    !props.tab.hasMore &&
    !props.tab.loadingMore &&
    filteredIdx.value.length >= prefs.result.maxTotalRows,
)

/** 生效的列筛选数量（含值筛选） */
const activeFilterCount = computed(
  () =>
    Object.values(colSettings.value).filter((s) => s.filter).length +
    Object.values(valueFilters.value).filter((v) => v.length > 0).length,
)

function isDateLike(s: SemanticType): boolean {
  return s === 'date' || s === 'datetime' || s === 'time'
}

function isNumberLike(s: SemanticType): boolean {
  return s === 'number' || s === 'decimal'
}

// ================= 统一数据管线：矩阵 + 索引（composable：useResultPipeline） =================
const { matrix, propIndex, filteredIdx, valueFilters, distinctCache, resultRows } =
  useResultPipeline({
    tab: () => props.tab,
    colSettings,
    searchApplied,
    editMode,
  })

// ================= 结果集增强（统计 / 列宽 / 长文卡 / 粘贴建行） =================

/** 列 distinct 值上限 */
const DISTINCT_CAP = 200

/** 列 distinct 值（cap 200，按原始矩阵计算，缓存至结果变更） */
function distinctOf(prop: string): string[] {
  const cached = distinctCache.value.get(prop)
  if (cached) return cached
  const col = propIndex.value.get(prop)
  const seen = new Set<string>()
  if (col !== undefined) {
    for (const row of matrix.value) {
      seen.add(cellText(row[col]))
      if (seen.size >= DISTINCT_CAP) break
    }
  }
  const list = [...seen].slice(0, DISTINCT_CAP)
  distinctCache.value.set(prop, list)
  return list
}

/** 值筛选确认（ColValueFilter 暂存后一次性提交） */
function setValueFilter(prop: string, values: string[]): void {
  if (values.length === 0) {
    const next = { ...valueFilters.value }
    delete next[prop]
    valueFilters.value = next
  } else {
    valueFilters.value = { ...valueFilters.value, [prop]: values }
  }
}

/** 数值列统计（对过滤后行计算） */
const showSummary = ref(false)
const summaryCols = computed(() => columns.value.filter((c) => isNumberLike(c.semantic)))
const summaryData = computed(() => {
  const out: { name: string; prop: string; sum: number; avg: number; min: number; max: number; n: number }[] = []
  for (const c of summaryCols.value) {
    let sum = 0
    let min = Number.POSITIVE_INFINITY
    let max = Number.NEGATIVE_INFINITY
    let n = 0
    for (const row of resultRows.value) {
      const v = row[c.prop]
      if (typeof v === 'number' && Number.isFinite(v)) {
        sum += v
        n += 1
        if (v < min) min = v
        if (v > max) max = v
      }
    }
    if (n > 0) out.push({ name: c.name, prop: c.prop, sum, avg: sum / n, min, max, n })
  }
  return out
})

function formatSum(n: number): string {
  if (!Number.isFinite(n)) return '-'
  if (Number.isInteger(n)) return n.toLocaleString()
  return Number(n.toPrecision(10)).toLocaleString(undefined, { maximumFractionDigits: 4 })
}

/** 列宽持久化（按 连接+库+表/SQL 键 localStorage） */
const colWidths = ref<Record<string, number>>({})
const widthKey = computed(() => {
  const tab = props.tab
  let hash = 5381
  const sql = tab.baseSql || tab.sql
  for (let i = 0; i < sql.length; i++) hash = ((hash << 5) + hash + sql.charCodeAt(i)) | 0
  return `w:${tab.connectionId}:${tab.database}:${tab.table ?? ''}:${hash}`
})

watch(
  widthKey,
  (key) => {
    try {
      const all = JSON.parse(localStorage.getItem('personaldb.colWidths') ?? '{}') as Record<string, Record<string, number>>
      colWidths.value = { ...(all[key] ?? {}) }
    } catch {
      colWidths.value = {}
    }
  },
  { immediate: true },
)

let widthSaveTimer: number | undefined
function onHeaderResize(prop: string | undefined, width: number): void {
  if (!prop) return
  colWidths.value = { ...colWidths.value, [prop]: Math.round(width) }
  window.clearTimeout(widthSaveTimer)
  widthSaveTimer = window.setTimeout(() => {
    try {
      const all = JSON.parse(localStorage.getItem('personaldb.colWidths') ?? '{}') as Record<string, Record<string, number>>
      all[widthKey.value] = { ...colWidths.value }
      localStorage.setItem('personaldb.colWidths', JSON.stringify(all))
    } catch {
      /* ignore */
    }
  }, 300)
}

/** 表头双击自适应列宽：按表头 + 前 200 行内容测宽（canvas 测量） */
const lastHeaderClick = ref<{ prop: string; at: number }>({ prop: '', at: 0 })
let measureCtx: CanvasRenderingContext2D | null = null

function onHeaderClick(column: { property?: string; label?: string }): void {
  const prop = column.property
  if (!prop) return
  const now = Date.now()
  if (lastHeaderClick.value.prop === prop && now - lastHeaderClick.value.at < 400) {
    lastHeaderClick.value = { prop: '', at: 0 }
    autoFitColumn(prop, column.label ?? '')
    return
  }
  lastHeaderClick.value = { prop, at: now }
}

function autoFitColumn(prop: string, headerLabel: string): void {
  const col = columns.value.find((c) => c.prop === prop)
  if (!col) return
  if (!measureCtx) {
    measureCtx = document.createElement('canvas').getContext('2d')
  }
  const ctx = measureCtx
  const fontSize = 12
  const width = (s: string): number => {
    if (!ctx) return s.length * 7
    ctx.font = `${fontSize}px Consolas, 'Courier New', monospace`
    return ctx.measureText(s).width
  }
  let max = width(headerLabel) + 30 // 排序图标/设置图标余量
  const rows = resultRows.value.slice(0, 200)
  for (const row of rows) {
    const v = row[prop]
    const s = v === null || v === undefined ? 'NULL' : String(v)
    if (s.length === 0) continue
    const w = width(s) + 26
    if (w > max) max = w
  }
  colWidths.value = { ...colWidths.value, [prop]: Math.min(600, Math.max(60, Math.ceil(max))) }
  onHeaderResize(prop, colWidths.value[prop]!)
}

/** 长文本/JSON 悬浮卡（>80 字符） */
const cellCardVisible = ref(false)
const cellCardAnchor = ref<HTMLElement | null>(null)
const cellCardText = ref('')

function onCellMouseEnter(row: Record<string, CellValue>, column: { property?: string }, _cell: unknown, ev: MouseEvent): void {
  if (editMode.value) return
  const col = columns.value.find((c) => c.prop === column.property)
  if (!col || (col.semantic !== 'longtext' && col.semantic !== 'json')) return
  const v = row[col.prop]
  const s = v === null || v === undefined ? '' : String(v)
  if (s.length <= 80) return
  cellCardText.value = s
  cellCardAnchor.value = ev.currentTarget as HTMLElement
  cellCardVisible.value = true
}

function onCellMouseLeave(): void {
  cellCardVisible.value = false
}

function copyCellCard(): void {
  void copyText(cellCardText.value)
  ElMessage.success(t('cell.copied'))
}

/** 编辑模式：从剪贴板粘贴 TSV 批量新增行 */
function onGridPaste(e: ClipboardEvent): void {
  if (!editMode.value || editingCell.value) return
  const text = e.clipboardData?.getData('text/plain') ?? ''
  if (!text || (!text.includes('\n') && !text.includes('\t'))) return
  const lines = text.split(/\r?\n/).filter((l) => l.trim() !== '')
  if (lines.length === 0) return
  e.preventDefault()
  pushEditUndo()
  const cols = columns.value
  let added = 0
  for (const line of lines) {
    const cells = line.split('\t')
    const values: Record<string, CellValue> = {}
    cols.forEach((c, i) => {
      const raw = cells[i] ?? ''
      values[c.prop] = raw === '' ? null : coerceValue(c.prop, raw)
    })
    editRows.value.push({ orig: {}, values, isNew: true, deleted: false })
    added += 1
  }
  ElMessage.success(t('result.pastedRows', { n: added }))
}

/** 错误重试 */
function retryRun(): void {
  void tabsStore.runTab(props.tab, props.tab.baseSql || props.tab.sql || undefined)
}

/** 历史双击：回填并立即执行 */
function runFromHistory(sql: string): void {
  props.tab.sql = sql
  void tabsStore.runTab(props.tab, sql)
}

/** 连接类错误 → 错误条显示「重连」而非「重试」 */
const errorIsConn = computed(() => looksLikeConnError(props.tab.error))

/** 执行超时 → 错误条显示「调整超时」直达首选项 */
const errorIsTimeout = computed(() => props.tab.errorKind === 'timeout')

function clearError(): void {
  props.tab.error = ''
  props.tab.errorKind = undefined
}

/** 打开首选项「查询执行」分类（放宽或关闭执行超时） */
function openTimeoutSettings(): void {
  dispatchCommand('open-settings', undefined, 'query')
}

/** 编辑态提示条文案 */
const editModeLabel = computed(() =>
  editHighRisk.value ? t('edit.modeAll') : t('edit.modePk'),
)
</script>

<style scoped>
.query-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.query-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
  /* 功能带（SPEC-UI-25）：浅底 + 下边线由 band 令牌统一 */
}

.query-toolbar :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

.query-toolbar :deep(.el-button) {
  flex-shrink: 0;
}

.db-select {
  width: 180px;
}

.hint {
  margin-left: auto;
  color: var(--el-text-color-placeholder);
  font-size: 12px;
}

.editor-wrap {
  overflow: hidden;
  flex-shrink: 0;
}

.splitter-h {
  flex: 0 0 5px;
  cursor: row-resize;
  background: transparent;
  border-bottom: 1px solid var(--el-border-color-lighter);
  transition: background-color 0.15s;
}
.splitter-h:hover {
  background: var(--el-color-primary-light-7);
}




.meta-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-left: 4px;
}

.col-vis {
  display: flex;
  flex-direction: column;
  max-height: 300px;
  overflow: auto;
}

.col-vis :deep(.el-checkbox) {
  margin-right: 0;
  height: 24px;
}

.col-vis-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.export-dir {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.dir-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.load-more-hint {
  text-align: center;
  padding: 4px 0;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.load-sql-hint {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.batch-hint {
  margin-left: 8px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.export-cols {
  width: 100%;
}

.export-col-list {
  max-height: 160px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  padding: 4px 8px;
  margin-top: 4px;
}

.export-col-list :deep(.el-checkbox) {
  height: 24px;
  margin-right: 0;
}

.is-editing :deep(.cell-wrap) {
  cursor: text;
}

.edit-cell {
  display: block;
  min-height: 18px;
  padding: 0 2px;
}

.edit-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
  padding: 4px 8px;
  font-size: 12px;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
}

.edit-risk {
  color: var(--el-color-warning);
}

.edit-bar .spacer {
  flex: 1;
}

.result-table :deep(.edit-new) {
  background: var(--el-color-success-light-9);
}

.result-table :deep(.edit-dirty) {
  background: var(--el-color-warning-light-9);
}

.result-table :deep(.edit-deleted) {
  background: var(--el-color-danger-light-9);
  text-decoration: line-through;
  color: var(--el-text-color-secondary);
}

.dml-preview {
  max-height: 380px;
  overflow: auto;
  margin: 0;
  padding: 8px 10px;
  font-size: 12px;
  font-family: Consolas, 'Courier New', monospace;
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--el-fill-color-light);
  border-radius: 4px;
}

.type-confirm {
  margin-top: 8px;
}
.tc-hint {
  font-size: 12px;
  color: var(--el-color-danger);
  margin-bottom: 6px;
}

.result-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
}

.result-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    row-gap: 4px;
    padding: 4px 8px;
    background: var(--pd-band-bg);
    border-bottom: var(--pd-band-border);
    }

/* 错误条（重试） */
.error-bar {
  --el-alert-padding: 6px 10px;
}
.error-bar .eb-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.error-bar .eb-text {
  flex: 1;
  min-width: 0;
  max-height: 72px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
  font-size: 12px;
}
.error-bar .eb-actions {
  flex-shrink: 0;
  display: inline-flex;
  gap: 4px;
}

/* 编辑态提示条 */
.edit-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 10px;
  margin-bottom: 4px;
  border-radius: 6px;
  font-size: 12px;
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning-dark-2);
  border: 1px solid var(--el-color-warning-light-7);
}
.edit-strip.is-risk {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
  border-color: var(--el-color-danger-light-7);
}
.edit-strip .es-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 统计条 */
.sum-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 2px 0;
  overflow-x: auto;
  flex-shrink: 0;
}
.sum-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--el-fill-color-light);
  white-space: nowrap;
}
.sum-chip .sum-name {
  font-weight: 600;
  opacity: 0.8;
}
.sum-chip .sum-val {
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
}

/* 列值筛选列表 */
.cs-values-row {
  margin-top: 2px;
}
.cs-values {
  max-height: 150px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px 2px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}

/* 长文悬浮卡（与 SQL 卡共用样式） */
.sql-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.sql-card-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.sql-card-title {
  font-size: 12px;
  font-weight: 600;
  opacity: 0.75;
}
.sql-card-body {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: Consolas, Menlo, monospace;
}

.result-search {
  width: 200px;
}

.match-count {
  font-size: 12px;
  color: var(--el-color-primary);
}

.col-settings {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.cs-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.cs-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

.col-fmt-btn.is-set {
  color: var(--el-color-primary);
}

.ctx-tag {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.result-table {
  flex: 1;
}

/* 紧凑布局：减小单元格内边距与行高 */
.result-table.is-compact :deep(.el-table__cell) {
  padding: 1px 0;
}

.result-table.is-compact :deep(.cell) {
  padding: 0 6px;
  line-height: 18px;
  font-size: 12px;
}

.plan-wrap {
  flex: 1;
  min-height: 0;
}

.col-head {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  width: 100%;
  padding-right: 14px;
}

.col-title {
  display: flex;
  align-items: center;
  gap: 4px;
  width: 100%;
  min-width: 0;
}

.col-name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-type {
  font-size: 10px;
  font-weight: 400;
  line-height: 1;
  color: var(--el-text-color-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

/* 列设置图标：hover 出现，已设置时常态显示 */
.col-fmt-btn {
  margin-left: auto;
  cursor: pointer;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  opacity: 0;
  transition: opacity 0.15s;
  flex-shrink: 0;
}

.col-head:hover .col-fmt-btn,
.col-fmt-btn.is-set {
  opacity: 1;
}

.col-fmt-btn:hover {
  color: var(--el-color-primary);
}

.result-placeholder {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
}
</style>
