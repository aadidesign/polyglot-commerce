// Package httpx contains shared HTTP helpers: an RFC 7807 error type, JSON
// encode/decode helpers, and the standard middleware (in middleware.go).
package httpx

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
)

// APIError is a domain error that knows its HTTP status and renders as
// application/problem+json (RFC 7807).
type APIError struct {
	Status int    `json:"status"`
	Title  string `json:"title"`
	Detail string `json:"detail"`
	Type   string `json:"type"`
}

func (e *APIError) Error() string { return e.Detail }

func problem(status int, detail string) *APIError {
	return &APIError{Status: status, Title: http.StatusText(status), Detail: detail,
		Type: "https://errors.ecommerce.dev/" + http.StatusText(status)}
}

func BadRequest(detail string) *APIError    { return problem(http.StatusBadRequest, detail) }
func Unauthorized() *APIError               { return problem(http.StatusUnauthorized, "authentication required") }
func Forbidden() *APIError                  { return problem(http.StatusForbidden, "insufficient permissions") }
func NotFound(resource string) *APIError    { return problem(http.StatusNotFound, resource+" not found") }
func Conflict(detail string) *APIError      { return problem(http.StatusConflict, detail) }
func TooManyRequests() *APIError            { return problem(http.StatusTooManyRequests, "rate limit exceeded") }
func Internal() *APIError                   { return problem(http.StatusInternalServerError, "an internal error occurred") }

// JSON writes a value as a JSON response.
func JSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if v != nil {
		_ = json.NewEncoder(w).Encode(v)
	}
}

// Error renders any error. Known *APIError values map to their status; anything
// else is logged and returned as a generic 500 (never leaking internals).
func Error(w http.ResponseWriter, r *http.Request, err error) {
	var ae *APIError
	if e, ok := err.(*APIError); ok {
		ae = e
	} else {
		slog.ErrorContext(r.Context(), "unhandled error", "error", err.Error())
		ae = Internal()
	}
	w.Header().Set("Content-Type", "application/problem+json")
	w.WriteHeader(ae.Status)
	_ = json.NewEncoder(w).Encode(ae)
}

// Decode reads a JSON body (capped at 1 MiB) into dst.
func Decode(r *http.Request, dst any) error {
	r.Body = http.MaxBytesReader(nil, r.Body, 1<<20)
	dec := json.NewDecoder(r.Body)
	dec.DisallowUnknownFields()
	if err := dec.Decode(dst); err != nil {
		return BadRequest(fmt.Sprintf("invalid JSON body: %v", err))
	}
	return nil
}
