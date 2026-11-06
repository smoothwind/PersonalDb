-- =============================================================================
-- OceanBase 容器：创建 Oracle 兼容模式租户（仅 root@sys 可执行）
-- ⚠️ OceanBase 社区版（oceanbase-ce）不支持 Oracle 兼容模式（企业版特性），
--    CREATE TENANT 报 1235 "Not support oracle mode"——本文件在 CE 上仅留档，
--    换企业版镜像后由 ob-init.sh 全自动执行（含等待、密码设置）。
-- 验证：PersonalDb 连 OceanBase Oracle，用户名 sys@test_oracle，密码 SysOracle_123，
--       Schema 留空，service_name 填 test_oracle。
-- 依据：OB 4.x CREATE TENANT——ob_compatibility_mode 只能在建租户时指定。
-- =============================================================================

-- 1. 资源规格（2G 内存 + 显式 2G 日志盘：默认 3×mem=6G 会超日志盘配额）
CREATE RESOURCE UNIT IF NOT EXISTS pdb_unit MAX_CPU 1, MIN_CPU 1, MEMORY_SIZE = '2G', LOG_DISK_SIZE = '2G';

-- 2. 资源池（单 zone 单副本）
CREATE RESOURCE POOL IF NOT EXISTS pdb_pool UNIT = 'pdb_unit', UNIT_NUM = 1, ZONE_LIST = ('zone1');

-- 3. Oracle 兼容模式租户（模式只能建时指定；白名单放开放通宿主机连接）
CREATE TENANT IF NOT EXISTS test_oracle
  PRIMARY_ZONE = 'zone1',
  RESOURCE_POOL_LIST = ('pdb_pool')
  SET ob_compatibility_mode = 'oracle',
      ob_tcp_invited_nodes = '%';

-- 4. 确认租户已激活（TENANT_STATUS 列应为 ACTIVE；刚创建时可能仍在 CREATING）
SELECT TENANT_NAME, TENANT_TYPE, TENANT_STATUS, compatibility_mode
  FROM DBA_OB_TENANTS WHERE TENANT_NAME = 'test_oracle';
