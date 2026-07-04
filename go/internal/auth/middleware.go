package auth

import (
	"context"
	"net/http"
	"strings"

	"github.com/google/uuid"

	"ecommerce/internal/httpx"
)

type ctxKey struct{}

// Authenticate verifies the bearer access token and stores the claims in the
// request context. Requests without a valid token are rejected with 401.
func (m *Manager) Authenticate(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		header := r.Header.Get("Authorization")
		token, ok := strings.CutPrefix(header, "Bearer ")
		if !ok || token == "" {
			httpx.Error(w, r, httpx.Unauthorized())
			return
		}
		claims, err := m.VerifyAccess(token)
		if err != nil {
			httpx.Error(w, r, httpx.Unauthorized())
			return
		}
		ctx := context.WithValue(r.Context(), ctxKey{}, claims)
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// Claims returns the authenticated claims, if present.
func Claims(ctx context.Context) (*AccessClaims, bool) {
	c, ok := ctx.Value(ctxKey{}).(*AccessClaims)
	return c, ok
}

// UserID returns the authenticated user's id.
func UserID(ctx context.Context) (uuid.UUID, error) {
	c, ok := Claims(ctx)
	if !ok {
		return uuid.Nil, httpx.Unauthorized()
	}
	id, err := uuid.Parse(c.Subject)
	if err != nil {
		return uuid.Nil, httpx.Unauthorized()
	}
	return id, nil
}

// Require returns an error unless the context holds the given permission.
func Require(ctx context.Context, permission string) error {
	c, ok := Claims(ctx)
	if !ok {
		return httpx.Unauthorized()
	}
	if !c.HasPermission(permission) {
		return httpx.Forbidden()
	}
	return nil
}
