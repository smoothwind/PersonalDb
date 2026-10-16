// oracle-bridge：PersonalDb 的 Oracle / OceanBase-Oracle 无 OCI 桥接进程。
//
// 使用 go-ora（纯 Go Oracle 协议实现，运行时无需 Instant Client），
// 通过 stdin/stdout 的 NDJSON（每行一个 JSON）与宿主 Rust 进程通信。
//
// 一个进程承载一个数据库连接（与客户端"连接 = 会话"模型一致）。
package main

import (
	"bufio"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	go_ora "github.com/sijms/go-ora/v2"
)

type request struct {
	ID       int      `json:"id"`
	Cmd      string   `json:"cmd"`
	Host     string   `json:"host"`
	Port     int      `json:"port"`
	Service  string   `json:"service"`
	User     string   `json:"user"`
	Password string   `json:"password"`
	SQL      string   `json:"sql"`
	Params   []string `json:"params"`
	MaxRows  int      `json:"maxRows"`
	AutoCommit *bool  `json:"autoCommit,omitempty"`
}

type columnOut struct {
	Name      string `json:"name"`
	TypeLabel string `json:"typeLabel"`
	Semantic  string `json:"semantic"`
}

type queryOut struct {
	Columns      []columnOut `json:"columns"`
	Rows         [][]any     `json:"rows"`
	AffectedRows *int64      `json:"affectedRows"`
	Truncated    bool        `json:"truncated"`
}

type response struct {
	ID    int             `json:"id"`
	OK    bool            `json:"ok"`
	Error string          `json:"error,omitempty"`
	Data  json.RawMessage `json:"data,omitempty"`
}

type bridge struct {
	db     *sql.DB
	tx     *sql.Tx
	manual bool
}

func (b *bridge) beginLocked() error {
	if b.tx != nil {
		return nil
	}
	tx, err := b.db.Begin()
	if err != nil {
		return err
	}
	b.tx = tx
	return nil
}

func (b *bridge) respond(enc *json.Encoder, id int, ok bool, errMsg string, data any) {
	r := response{ID: id, OK: ok, Error: errMsg}
	if ok && data != nil {
		raw, err := json.Marshal(data)
		if err != nil {
			r.OK = false
			r.Error = err.Error()
		} else {
			r.Data = raw
		}
	}
	_ = enc.Encode(r)
}

func (b *bridge) handle(req request) (any, error) {
	switch req.Cmd {
	case "ping":
		return map[string]any{"pong": true}, nil

	case "connect":
		if b.db != nil {
			_ = b.db.Close()
		}
		url := go_ora.BuildUrl(req.Host, req.Port, req.Service, req.User, req.Password, nil)
		db, err := sql.Open("oracle", url)
		if err != nil {
			return nil, err
		}
		db.SetMaxOpenConns(1)
		if err := db.Ping(); err != nil {
			_ = db.Close()
			return nil, err
		}
		b.db = db
		b.manual = false
		b.tx = nil
		return map[string]any{"connected": true}, nil

	case "serverVersion":
		if b.db == nil {
			return nil, fmt.Errorf("not connected")
		}
		var banner string
		row := b.db.QueryRow("SELECT banner FROM v$version WHERE ROWNUM = 1")
		if err := row.Scan(&banner); err != nil {
			// 部分 OB 租户无 v$version 权限，降级
			var ver string
			if e2 := b.db.QueryRow("SELECT version FROM v$instance").Scan(&ver); e2 == nil {
				return map[string]any{"version": ver}, nil
			}
			return map[string]any{"version": "Oracle (未知版本)"}, nil
		}
		return map[string]any{"version": banner}, nil

	case "query", "exec":
		if b.db == nil {
			return nil, fmt.Errorf("not connected")
		}
		args := make([]any, len(req.Params))
		for i, p := range req.Params {
			args[i] = p
		}
		upper := strings.ToUpper(strings.TrimSpace(req.SQL))
		isQuery := strings.HasPrefix(upper, "SELECT") || strings.HasPrefix(upper, "WITH") ||
			strings.HasPrefix(upper, "EXPLAIN") || strings.HasPrefix(upper, "DESC")

		if !isQuery {
			// DML / DDL：取影响行数
			var res sql.Result
			var err error
			if b.tx != nil {
				res, err = b.tx.Exec(req.SQL, args...)
			} else {
				res, err = b.db.Exec(req.SQL, args...)
			}
			if err != nil {
				return nil, err
			}
			n, _ := res.RowsAffected()
			return queryOut{Columns: []columnOut{}, Rows: [][]any{}, AffectedRows: &n}, nil
		}

		q := b.db.Query
		if b.tx != nil {
			q = b.tx.Query
		}
		rows, err := q(req.SQL, args...)
		if err != nil {
			return nil, err
		}
		defer rows.Close()

		colTypes, err := rows.ColumnTypes()
		if err != nil {
			return nil, err
		}
		cols := make([]columnOut, len(colTypes))
		for i, ct := range colTypes {
			dbType := strings.ToUpper(ct.DatabaseTypeName())
			cols[i] = columnOut{Name: ct.Name(), TypeLabel: dbType, Semantic: semanticOf(dbType)}
		}

		out := queryOut{Columns: cols, Rows: [][]any{}}
		maxRows := req.MaxRows
		if maxRows <= 0 {
			maxRows = 1000
		}
		for rows.Next() {
			if len(out.Rows) >= maxRows {
				out.Truncated = true
				break
			}
			vals := make([]any, len(cols))
			ptrs := make([]any, len(cols))
			for i := range vals {
				ptrs[i] = &vals[i]
			}
			if err := rows.Scan(ptrs...); err != nil {
				return nil, err
			}
			line := make([]any, len(cols))
			for i := range vals {
				line[i] = convert(vals[i], cols[i].Semantic)
			}
			out.Rows = append(out.Rows, line)
		}
		if err := rows.Err(); err != nil {
			return nil, err
		}
		return out, nil

	case "begin":
		if b.db == nil {
			return nil, fmt.Errorf("not connected")
		}
		b.manual = true
		return nil, b.beginLocked()

	case "commit", "rollback":
		if b.db == nil {
			return nil, fmt.Errorf("not connected")
		}
		if b.tx != nil {
			var err error
			if req.Cmd == "commit" {
				err = b.tx.Commit()
			} else {
				err = b.tx.Rollback()
			}
			b.tx = nil
			if err != nil {
				return nil, err
			}
		}
		// 手动模式：提交/回滚后立即开启新事务窗口
		if b.manual {
			return nil, b.beginLocked()
		}
		return nil, nil

	case "autocommit":
		if b.db == nil {
			return nil, fmt.Errorf("not connected")
		}
		enabled := req.AutoCommit != nil && *req.AutoCommit
		if enabled {
			// 关闭手动模式：提交未决变更
			if b.tx != nil {
				if err := b.tx.Commit(); err != nil {
					return nil, err
				}
				b.tx = nil
			}
			b.manual = false
		} else {
			// 进入手动模式：开启事务窗口
			b.manual = true
			if err := b.beginLocked(); err != nil {
				return nil, err
			}
		}
		return nil, nil

	case "close":
		if b.tx != nil {
			_ = b.tx.Rollback()
			b.tx = nil
		}
		if b.db != nil {
			_ = b.db.Close()
			b.db = nil
		}
		return nil, nil
	}
	return nil, fmt.Errorf("unknown command: %s", req.Cmd)
}

func semanticOf(dbType string) string {
	switch {
	case strings.Contains(dbType, "NUMBER"),
		strings.Contains(dbType, "FLOAT"),
		strings.Contains(dbType, "DECIMAL"),
		strings.Contains(dbType, "INTEGER"),
		strings.Contains(dbType, "BINARY_DOUBLE"),
		strings.Contains(dbType, "BINARY_FLOAT"):
		return "decimal"
	case strings.Contains(dbType, "CLOB"), strings.Contains(dbType, "LONG"):
		return "longtext"
	case strings.Contains(dbType, "BLOB"), strings.Contains(dbType, "RAW"), strings.Contains(dbType, "BFILE"):
		return "binary"
	case strings.Contains(dbType, "TIMESTAMP"), dbType == "DATE":
		return "datetime"
	case strings.Contains(dbType, "JSON"):
		return "json"
	case strings.Contains(dbType, "CHAR"), strings.Contains(dbType, "VARCHAR"), strings.Contains(dbType, "ROWID"):
		return "text"
	}
	return "unknown"
}

func convert(v any, semantic string) any {
	switch t := v.(type) {
	case nil:
		return nil
	case []byte:
		if semantic == "binary" {
			return base64.StdEncoding.EncodeToString(t)
		}
		return string(t)
	case time.Time:
		return t.Format("2006-01-02 15:04:05.999999")
	case bool, int64, float64, string:
		return t
	default:
		return fmt.Sprint(t)
	}
}

func main() {
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 1024*1024), 64*1024*1024)
	enc := json.NewEncoder(os.Stdout)
	b := &bridge{}

	for scanner.Scan() {
		line := scanner.Bytes()
		if len(strings.TrimSpace(string(line))) == 0 {
			continue
		}
		var req request
		if err := json.Unmarshal(line, &req); err != nil {
			b.respond(enc, 0, false, "invalid json: "+err.Error(), nil)
			continue
		}
		if req.Cmd == "shutdown" {
			break
		}
		data, err := b.handle(req)
		if err != nil {
			b.respond(enc, req.ID, false, err.Error(), nil)
			continue
		}
		b.respond(enc, req.ID, true, "", data)
	}
}
