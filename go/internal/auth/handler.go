package auth

import (
	"context"
	"errors"
	"net/http"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
	"github.com/jackc/pgx/v5/pgxpool"

	"ecommerce/internal/db"
	"ecommerce/internal/httpx"
)

type Handlers struct {
	Pool *pgxpool.Pool
	Mgr  *Manager
}

func isUniqueViolation(err error) bool {
	var pgErr *pgconn.PgError
	return errors.As(err, &pgErr) && pgErr.Code == "23505"
}

func (h *Handlers) issueSession(ctx context.Context, q db.Querier, userID, family uuid.UUID) (TokenResponse, error) {
	roles, err := Roles(ctx, q, userID)
	if err != nil {
		return TokenResponse{}, err
	}
	perms, err := Permissions(ctx, q, userID)
	if err != nil {
		return TokenResponse{}, err
	}
	access, err := h.Mgr.IssueAccess(userID, roles, perms)
	if err != nil {
		return TokenResponse{}, err
	}
	refresh, err := h.Mgr.IssueRefresh(userID, family)
	if err != nil {
		return TokenResponse{}, err
	}
	if err := StoreRefresh(ctx, q, refresh.JTI, family, userID, refresh.ExpiresAt); err != nil {
		return TokenResponse{}, err
	}
	return TokenResponse{
		AccessToken:  access.Token,
		RefreshToken: refresh.Token,
		TokenType:    "Bearer",
		ExpiresIn:    int64(time.Until(access.ExpiresAt).Seconds()),
	}, nil
}

func (h *Handlers) Register(w http.ResponseWriter, r *http.Request) {
	var req RegisterRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := req.Validate(); err != nil {
		httpx.Error(w, r, err)
		return
	}
	hash, err := HashPassword(req.Password)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}

	ctx := r.Context()
	tx, err := h.Pool.Begin(ctx)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	defer tx.Rollback(ctx)

	user, err := CreateUser(ctx, tx, req.Email, hash, req.FullName)
	if err != nil {
		if isUniqueViolation(err) {
			httpx.Error(w, r, httpx.Conflict("email already registered"))
			return
		}
		httpx.Error(w, r, err)
		return
	}
	if err := AssignRole(ctx, tx, user.ID, "customer"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	tokens, err := h.issueSession(ctx, tx, user.ID, uuid.New())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := tx.Commit(ctx); err != nil {
		httpx.Error(w, r, err)
		return
	}

	roles, _ := Roles(ctx, h.Pool, user.ID)
	httpx.JSON(w, http.StatusCreated, RegisterResponse{
		User:   newUserResponse(user, roles),
		Tokens: tokens,
	})
}

func (h *Handlers) Login(w http.ResponseWriter, r *http.Request) {
	var req LoginRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	ctx := r.Context()
	user, err := FindByEmail(ctx, h.Pool, req.Email)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if !VerifyPassword(req.Password, user.PasswordHash) {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	}
	if user.Status != "active" {
		httpx.Error(w, r, httpx.Forbidden())
		return
	}
	tokens, err := h.issueSession(ctx, h.Pool, user.ID, uuid.New())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusOK, tokens)
}

func (h *Handlers) Refresh(w http.ResponseWriter, r *http.Request) {
	var req RefreshRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	claims, err := h.Mgr.VerifyRefresh(req.RefreshToken)
	if err != nil {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	}
	jti, err1 := uuid.Parse(claims.ID)
	family, err2 := uuid.Parse(claims.Family)
	if err1 != nil || err2 != nil {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	}

	ctx := r.Context()
	rec, err := GetRefresh(ctx, h.Pool, jti)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if rec.Revoked {
		httpx.Error(w, r, httpx.Unauthorized())
		return
	}
	if rec.Used {
		// Reuse detected - revoke the whole family.
		_ = RevokeFamily(ctx, h.Pool, family)
		httpx.Error(w, r, httpx.Unauthorized())
		return
	}

	tx, err := h.Pool.Begin(ctx)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	defer tx.Rollback(ctx)
	if err := MarkRefreshUsed(ctx, tx, jti); err != nil {
		httpx.Error(w, r, err)
		return
	}
	tokens, err := h.issueSession(ctx, tx, rec.UserID, family)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := tx.Commit(ctx); err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusOK, tokens)
}

func (h *Handlers) Logout(w http.ResponseWriter, r *http.Request) {
	var req LogoutRequest
	if err := httpx.Decode(r, &req); err == nil {
		if claims, err := h.Mgr.VerifyRefresh(req.RefreshToken); err == nil {
			if family, err := uuid.Parse(claims.Family); err == nil {
				_ = RevokeFamily(r.Context(), h.Pool, family)
			}
		}
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h *Handlers) Me(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	userID, err := UserID(ctx)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	user, err := FindByID(ctx, h.Pool, userID)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.NotFound("user"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	roles, _ := Roles(ctx, h.Pool, userID)
	httpx.JSON(w, http.StatusOK, newUserResponse(user, roles))
}
