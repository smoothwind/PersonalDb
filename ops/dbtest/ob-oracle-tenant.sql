-- =============================================================================
-- OceanBase CE 容器：创建 Oracle 兼容模式租户（仅 root@sys 可执行）
-- 用法（容器就绪后，在 WSL/仓库根目录）：
--   docker exec -i pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot < ops/dbtest/ob-oracle-tenant.sql
-- 之后等待约 30s 租户就绪，再设管理员密码（须登进新租户执行）：
--   docker exec -it pdb-ob obclient -h127.0.0.1 -P2881 -usys@test_oracle -A \
--     -e "ALTER USER sys IDENTIFIED BY 'SysOracle_123'"
-- 验证：PersonalDb 连 OceanBase Oracle，用户名 sys@test_oracle，密码 SysOracle_123，
--       Schema 留空，service_name 填 test_oracle。
-- 依据：OB CE 4.x 官方文档 CREATE TENANT——ob_compatibility_mode 只能在建租户时指定。
-- =============================================================================

-- 1. 资源规格（1C/2G，MINI 档 6G 内存限额下够用）
CREATE RESOURCE UNIT IF NOT EXISTS pdb_unit MAX_CPU 1, MEMORY_SIZE = '2G';

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
