// Package pagination implements opaque keyset (cursor) pagination, avoiding
// SQL OFFSET on hot paths.
package pagination

import (
	"encoding/base64"
	"encoding/json"
	"strconv"
)

const (
	Default = 20
	Max     = 100
)

// Cursor encodes the last-seen sort position (timestamp + id).
type Cursor struct {
	TS string `json:"ts"`
	ID string `json:"id"`
}

func (c Cursor) Encode() string {
	b, _ := json.Marshal(c)
	return base64.RawURLEncoding.EncodeToString(b)
}

func Decode(raw string) (Cursor, error) {
	var c Cursor
	b, err := base64.RawURLEncoding.DecodeString(raw)
	if err != nil {
		return c, err
	}
	if err := json.Unmarshal(b, &c); err != nil {
		return c, err
	}
	return c, nil
}

// ClampLimit parses a limit query param and bounds it to [1, Max].
func ClampLimit(raw string) int {
	n, err := strconv.Atoi(raw)
	if err != nil || n <= 0 {
		return Default
	}
	if n > Max {
		return Max
	}
	return n
}

// Page is the envelope returned for any paginated collection.
type Page[T any] struct {
	Items      []T     `json:"items"`
	NextCursor *string `json:"next_cursor"`
	HasMore    bool    `json:"has_more"`
}

func NewPage[T any](items []T, next *string) Page[T] {
	return Page[T]{Items: items, NextCursor: next, HasMore: next != nil}
}
