package personaldb;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.io.BufferedReader;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.sql.Blob;
import java.sql.Clob;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.Statement;
import java.sql.Timestamp;
import java.sql.Types;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Properties;

/**
 * 通用 JDBC 桥：以 NDJSON over stdio 与宿主（Rust）通信。
 *
 * classpath = jdbc-bridge.jar（含内置 OceanBase 驱动 + Gson） + 用户自定义驱动 jar。
 * 连接方式：{@code java -cp "<jdbc-bridge.jar>;<driver1.jar>;..." personaldb.JdbcBridge}。
 */
public final class JdbcBridge {
    private final Gson gson = new Gson();
    private Connection conn;
    private boolean manual = false;

    public static void main(String[] args) throws Exception {
        new JdbcBridge().run();
    }

    private void run() throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        String line;
        String cmd = null;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) {
                continue;
            }
            JsonObject resp = new JsonObject();
            long id = 0;
            try {
                JsonObject req = JsonParser.parseString(line).getAsJsonObject();
                if (req.has("id") && !req.get("id").isJsonNull()) {
                    id = req.get("id").getAsLong();
                }
                cmd = req.has("cmd") ? req.get("cmd").getAsString() : "";
                resp.addProperty("id", id);
                JsonElement data = handle(req, cmd);
                resp.addProperty("ok", true);
                resp.add("data", data == null ? JsonNull.INSTANCE : data);
            } catch (Throwable e) {
                resp = new JsonObject();
                resp.addProperty("id", id);
                resp.addProperty("ok", false);
                resp.addProperty("error", errorMessage(e));
            }
            out.println(gson.toJson(resp));
            out.flush();
            if ("shutdown".equals(cmd)) {
                break;
            }
        }
        closeQuietly();
    }

    private JsonElement handle(JsonObject req, String cmd) throws Exception {
        switch (cmd) {
            case "ping":
                return obj("pong", true);
            case "connect":
                connect(req);
                return obj("connected", true);
            case "serverVersion": {
                requireConn();
                DatabaseMetaData md = conn.getMetaData();
                String name = md.getDatabaseProductName();
                String ver = md.getDatabaseProductVersion();
                return obj("version", (name == null ? "" : name + " ") + (ver == null ? "" : ver));
            }
            case "query":
            case "exec":
                return query(req);
            case "begin":
                requireConn();
                if (manual) {
                    commitInternal();
                }
                manual = true;
                conn.setAutoCommit(false);
                return null;
            case "commit":
                requireConn();
                conn.commit();
                return null;
            case "rollback":
                requireConn();
                conn.rollback();
                return null;
            case "autocommit": {
                requireConn();
                boolean enabled = req.has("autoCommit") && req.get("autoCommit").getAsBoolean();
                if (enabled) {
                    if (manual) {
                        conn.commit();
                        conn.setAutoCommit(true);
                        manual = false;
                    }
                } else if (!manual) {
                    manual = true;
                    conn.setAutoCommit(false);
                }
                return null;
            }
            case "close":
            case "shutdown":
                closeQuietly();
                return null;
            // ---- 通用元数据（JDBC DatabaseMetaData） ----
            case "metaSchemas":
                return metaSchemas();
            case "metaCatalogs":
                return metaCatalogs();
            case "metaTables":
                return metaTables(req);
            case "metaColumns":
                return metaColumns(req);
            case "metaIndexes":
                return metaIndexes(req);
            case "metaPrimaryKeys":
                return metaPrimaryKeys(req);
            case "metaProcedures":
                return metaProcedures(req, false);
            case "metaFunctions":
                return metaProcedures(req, true);
            case "scanDriverClasses":
                return scanDriverClasses();
            default:
                throw new IllegalArgumentException("unknown command: " + cmd);
        }
    }

    private void connect(JsonObject req) throws Exception {
        closeQuietly();
        String url = str(req, "url");
        String driverClass = str(req, "driverClass");
        if (driverClass != null && !driverClass.isEmpty()) {
            Class.forName(driverClass);
        }
        if (url == null || url.isEmpty()) {
            // 向后兼容：由 host/port/service 构造 OceanBase URL
            String host = str(req, "host");
            int port = req.has("port") && !req.get("port").isJsonNull() ? req.get("port").getAsInt() : 2883;
            String service = str(req, "service");
            StringBuilder sb = new StringBuilder("jdbc:oceanbase://").append(host).append(':').append(port).append('/');
            if (service != null && !service.isEmpty()) {
                sb.append(service);
            }
            url = sb.toString();
        }
        Properties props = new Properties();
        String user = str(req, "user");
        String password = str(req, "password");
        if (user != null) {
            props.setProperty("user", user);
        }
        if (password != null) {
            props.setProperty("password", password);
        }
        if (req.has("props") && req.get("props").isJsonObject()) {
            for (var e : req.getAsJsonObject("props").entrySet()) {
                props.setProperty(e.getKey(), e.getValue().getAsString());
            }
        }
        if (!props.containsKey("connectTimeout")) {
            props.setProperty("connectTimeout", "15000");
        }
        if (!props.containsKey("socketTimeout")) {
            props.setProperty("socketTimeout", "60000");
        }
        conn = DriverManager.getConnection(url, props);
        manual = false;
    }

    private JsonElement query(JsonObject req) throws Exception {
        requireConn();
        String sql = str(req, "sql");
        List<String> params = new ArrayList<>();
        if (req.has("params") && req.get("params").isJsonArray()) {
            for (JsonElement p : req.getAsJsonArray("params")) {
                params.add(p.isJsonNull() ? null : p.getAsString());
            }
        }
        int maxRows = req.has("maxRows") && !req.get("maxRows").isJsonNull() ? req.get("maxRows").getAsInt() : 1000;
        if (maxRows <= 0) {
            maxRows = Integer.MAX_VALUE;
        }
        if (params.isEmpty()) {
            try (Statement st = conn.createStatement()) {
                st.setMaxRows(maxRows == Integer.MAX_VALUE ? 0 : maxRows + 1);
                if (st.execute(sql)) {
                    return readResultSet(st.getResultSet(), maxRows);
                }
                int affected = st.getUpdateCount();
                return emptyResult(affected);
            }
        }
        // Oracle 风格 :N 占位符 → JDBC ?，并按出现顺序重排参数（保留序号语义）
        StringBuilder sb = new StringBuilder();
        java.util.List<String> bound = new ArrayList<>();
        java.util.regex.Matcher m = java.util.regex.Pattern.compile(":(\\d+)").matcher(sql);
        int last = 0;
        while (m.find()) {
            sb.append(sql, last, m.start()).append('?');
            int idx = Integer.parseInt(m.group(1)) - 1;
            bound.add(idx >= 0 && idx < params.size() ? params.get(idx) : null);
            last = m.end();
        }
        sb.append(sql.substring(last));
        String jdbcSql = sb.toString();
        try (PreparedStatement ps = conn.prepareStatement(jdbcSql)) {
            ps.setMaxRows(maxRows == Integer.MAX_VALUE ? 0 : maxRows + 1);
            for (int i = 0; i < bound.size(); i++) {
                ps.setString(i + 1, bound.get(i));
            }
            if (ps.execute()) {
                return readResultSet(ps.getResultSet(), maxRows);
            }
            return emptyResult(ps.getUpdateCount());
        }
    }

    private JsonElement readResultSet(ResultSet rs, int maxRows) throws Exception {
        ResultSetMetaData md = rs.getMetaData();
        int n = md.getColumnCount();
        JsonArray columns = new JsonArray();
        String[] semantics = new String[n];
        for (int i = 1; i <= n; i++) {
            String typeLabel = md.getColumnTypeName(i);
            String semantic = semanticOf(md.getColumnType(i), typeLabel);
            semantics[i - 1] = semantic;
            JsonObject col = new JsonObject();
            col.addProperty("name", md.getColumnLabel(i));
            col.addProperty("typeLabel", typeLabel);
            col.addProperty("semantic", semantic);
            columns.add(col);
        }
        JsonArray rows = new JsonArray();
        int count = 0;
        boolean truncated = false;
        while (rs.next()) {
            if (count >= maxRows) {
                truncated = true;
                break;
            }
            JsonArray row = new JsonArray();
            for (int i = 1; i <= n; i++) {
                row.add(convert(rs.getObject(i), semantics[i - 1]));
            }
            rows.add(row);
            count++;
        }
        JsonObject o = new JsonObject();
        o.add("columns", columns);
        o.add("rows", rows);
        o.addProperty("affectedRows", 0);
        o.addProperty("truncated", truncated);
        return o;
    }

    private JsonObject emptyResult(int affected) {
        JsonObject o = new JsonObject();
        o.add("columns", new JsonArray());
        o.add("rows", new JsonArray());
        o.addProperty("affectedRows", affected < 0 ? 0 : affected);
        o.addProperty("truncated", false);
        return o;
    }

    // ================= 元数据 =================

    private JsonElement metaSchemas() throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        try (ResultSet rs = conn.getMetaData().getSchemas()) {
            while (rs.next()) {
                rows.add(new String[]{ rs.getString("TABLE_SCHEM") });
            }
        }
        return metadataResult(new String[]{ "SCHEMA" }, rows);
    }

    private JsonElement metaCatalogs() throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        try (ResultSet rs = conn.getMetaData().getCatalogs()) {
            while (rs.next()) {
                rows.add(new String[]{ rs.getString("TABLE_CAT") });
            }
        }
        return metadataResult(new String[]{ "CATALOG" }, rows);
    }

    private JsonElement metaTables(JsonObject req) throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        DatabaseMetaData md = conn.getMetaData();
        String catalog = str(req, "catalog");
        String schema = str(req, "schema");
        String pattern = str(req, "pattern");
        if (pattern == null || pattern.isEmpty()) {
            pattern = "%";
        }
        String[] types = { "TABLE", "VIEW" };
        try (ResultSet rs = md.getTables(catalog, schema, pattern, types)) {
            while (rs.next()) {
                rows.add(new String[]{
                        rs.getString("TABLE_CAT"),
                        rs.getString("TABLE_SCHEM"),
                        rs.getString("TABLE_NAME"),
                        rs.getString("TABLE_TYPE"),
                        rs.getString("REMARKS")
                });
            }
        }
        return metadataResult(new String[]{ "TABLE_CAT", "TABLE_SCHEM", "TABLE_NAME", "TABLE_TYPE", "REMARKS" }, rows);
    }

    private JsonElement metaColumns(JsonObject req) throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        DatabaseMetaData md = conn.getMetaData();
        try (ResultSet rs = md.getColumns(str(req, "catalog"), str(req, "schema"), str(req, "table"), null)) {
            while (rs.next()) {
                rows.add(new String[]{
                        rs.getString("COLUMN_NAME"),
                        rs.getString("TYPE_NAME"),
                        rs.getString("DATA_TYPE"),
                        rs.getString("NULLABLE"),
                        rs.getString("COLUMN_DEF"),
                        rs.getString("REMARKS"),
                        rs.getString("ORDINAL_POSITION")
                });
            }
        }
        return metadataResult(new String[]{
                "COLUMN_NAME", "TYPE_NAME", "DATA_TYPE", "NULLABLE", "COLUMN_DEF", "REMARKS", "ORDINAL_POSITION"
        }, rows);
    }

    private JsonElement metaIndexes(JsonObject req) throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        DatabaseMetaData md = conn.getMetaData();
        try (ResultSet rs = md.getIndexInfo(str(req, "catalog"), str(req, "schema"), str(req, "table"), false, false)) {
            while (rs.next()) {
                rows.add(new String[]{
                        rs.getString("INDEX_NAME"),
                        rs.getString("NON_UNIQUE"),
                        rs.getString("COLUMN_NAME"),
                        rs.getString("ORDINAL_POSITION"),
                        rs.getString("TYPE")
                });
            }
        }
        return metadataResult(new String[]{
                "INDEX_NAME", "NON_UNIQUE", "COLUMN_NAME", "ORDINAL_POSITION", "TYPE"
        }, rows);
    }

    private JsonElement metaPrimaryKeys(JsonObject req) throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        DatabaseMetaData md = conn.getMetaData();
        try (ResultSet rs = md.getPrimaryKeys(str(req, "catalog"), str(req, "schema"), str(req, "table"))) {
            while (rs.next()) {
                rows.add(new String[]{
                        rs.getString("COLUMN_NAME"),
                        rs.getString("KEY_SEQ"),
                        rs.getString("PK_NAME")
                });
            }
        }
        return metadataResult(new String[]{ "COLUMN_NAME", "KEY_SEQ", "PK_NAME" }, rows);
    }

    private JsonElement metaProcedures(JsonObject req, boolean functions) throws Exception {
        requireConn();
        List<String[]> rows = new ArrayList<>();
        DatabaseMetaData md = conn.getMetaData();
        String catalog = str(req, "catalog");
        String schema = str(req, "schema");
        String pattern = str(req, "pattern");
        if (pattern == null || pattern.isEmpty()) {
            pattern = "%";
        }
        ResultSet rs = functions
                ? md.getFunctions(catalog, schema, pattern)
                : md.getProcedures(catalog, schema, pattern);
        try (rs) {
            boolean fn = functions;
            while (rs.next()) {
                rows.add(new String[]{
                        rs.getString(fn ? "FUNCTION_CAT" : "PROCEDURE_CAT"),
                        rs.getString(fn ? "FUNCTION_SCHEM" : "PROCEDURE_SCHEM"),
                        rs.getString(fn ? "FUNCTION_NAME" : "PROCEDURE_NAME"),
                        rs.getString("REMARKS")
                });
            }
        }
        return metadataResult(new String[]{ "CAT", "SCHEM", "NAME", "REMARKS" }, rows);
    }

    /** 扫描当前 classpath 中注册的 JDBC 驱动类（ServiceLoader） */
    private JsonElement scanDriverClasses() {
        java.util.LinkedHashSet<String> classes = new java.util.LinkedHashSet<>();
        try {
            for (java.sql.Driver d : java.util.ServiceLoader.load(java.sql.Driver.class)) {
                classes.add(d.getClass().getName());
            }
        } catch (Throwable ignored) {
            // ignore
        }
        JsonArray arr = new JsonArray();
        for (String c : classes) {
            arr.add(new JsonPrimitive(c));
        }
        JsonObject o = new JsonObject();
        o.add("classes", arr);
        return o;
    }

    private JsonObject metadataResult(String[] colNames, List<String[]> rows) {        JsonArray columns = new JsonArray();
        for (String name : colNames) {
            JsonObject col = new JsonObject();
            col.addProperty("name", name);
            col.addProperty("typeLabel", "VARCHAR");
            col.addProperty("semantic", "text");
            columns.add(col);
        }
        JsonArray arr = new JsonArray();
        for (String[] r : rows) {
            JsonArray row = new JsonArray();
            for (String v : r) {
                row.add(v == null ? JsonNull.INSTANCE : new JsonPrimitive(v));
            }
            arr.add(row);
        }
        JsonObject o = new JsonObject();
        o.add("columns", columns);
        o.add("rows", arr);
        o.addProperty("affectedRows", 0);
        o.addProperty("truncated", false);
        return o;
    }

    // ================= 值转换 =================

    private static String semanticOf(int sqlType, String typeName) {
        String t = typeName == null ? "" : typeName.toUpperCase();
        if (t.contains("JSON")) {
            return "json";
        }
        switch (sqlType) {
            case Types.BOOLEAN:
            case Types.BIT:
                return "bool";
            case Types.DATE:
                return "date";
            case Types.TIME:
                return "time";
            case Types.TIMESTAMP:
            case Types.TIMESTAMP_WITH_TIMEZONE:
                return "datetime";
            case Types.TINYINT:
            case Types.SMALLINT:
            case Types.INTEGER:
            case Types.BIGINT:
            case Types.FLOAT:
            case Types.DOUBLE:
            case Types.REAL:
                return "number";
            case Types.DECIMAL:
            case Types.NUMERIC:
                return "decimal";
            case Types.BINARY:
            case Types.VARBINARY:
            case Types.LONGVARBINARY:
            case Types.BLOB:
                return "binary";
            case Types.CLOB:
            case Types.NCLOB:
            case Types.LONGVARCHAR:
            case Types.LONGNVARCHAR:
                return "longtext";
            default:
                return "text";
        }
    }

    private static JsonElement convert(Object v, String semantic) throws Exception {
        if (v == null) {
            return JsonNull.INSTANCE;
        }
        if (v instanceof Boolean) {
            return new JsonPrimitive((Boolean) v);
        }
        if (v instanceof BigDecimal || v instanceof BigInteger) {
            return new JsonPrimitive((Number) v);
        }
        if (v instanceof Number) {
            return new JsonPrimitive((Number) v);
        }
        if (v instanceof byte[]) {
            return new JsonPrimitive(Base64.getEncoder().encodeToString((byte[]) v));
        }
        if (v instanceof Timestamp) {
            return new JsonPrimitive(new SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSSSSS").format((Timestamp) v));
        }
        if (v instanceof java.sql.Date || v instanceof java.sql.Time) {
            return new JsonPrimitive(v.toString());
        }
        if (v instanceof Clob) {
            Clob c = (Clob) v;
            return new JsonPrimitive(c.getSubString(1, (int) Math.min(c.length(), Integer.MAX_VALUE)));
        }
        if (v instanceof Blob) {
            Blob b = (Blob) v;
            byte[] bytes = b.getBytes(1, (int) Math.min(b.length(), Integer.MAX_VALUE));
            return new JsonPrimitive(Base64.getEncoder().encodeToString(bytes));
        }
        return new JsonPrimitive(v.toString());
    }

    private void commitInternal() throws SQLException {
        if (conn != null && !conn.getAutoCommit()) {
            conn.commit();
        }
    }

    private void requireConn() throws SQLException {
        if (conn == null) {
            throw new SQLException("not connected");
        }
    }

    private void closeQuietly() {
        if (conn != null) {
            try {
                if (!conn.getAutoCommit()) {
                    conn.rollback();
                }
            } catch (Exception ignored) {
                // ignore
            }
            try {
                conn.close();
            } catch (Exception ignored) {
                // ignore
            }
            conn = null;
        }
        manual = false;
    }

    private static String errorMessage(Throwable e) {
        String msg = e.getMessage();
        if (msg == null || msg.isEmpty()) {
            msg = e.toString();
        }
        Throwable cause = e.getCause();
        if (cause != null && cause != e) {
            String cm = cause.getMessage();
            if (cm != null && !cm.isEmpty() && !msg.contains(cm)) {
                msg = msg + " | " + cm;
            }
        }
        return msg;
    }

    private static String str(JsonObject o, String key) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : null;
    }

    private static JsonObject obj(String key, Object value) {
        JsonObject o = new JsonObject();
        if (value instanceof Boolean) {
            o.addProperty(key, (Boolean) value);
        } else {
            o.addProperty(key, String.valueOf(value));
        }
        return o;
    }
}
