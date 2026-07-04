package auth

import (
	"context"
	"time"

	"github.com/google/uuid"

	"ecommerce/internal/db"
)

const userCols = "id, email, password_hash, full_name, email_verified, status, created_at"

func scanUser(row interface{ Scan(...any) error }) (User, error) {
	var u User
	err := row.Scan(&u.ID, &u.Email, &u.PasswordHash, &u.FullName, &u.EmailVerified, &u.Status, &u.CreatedAt)
	return u, err
}

func CreateUser(ctx context.Context, q db.Querier, email, hash, fullName string) (User, error) {
	row := q.QueryRow(ctx,
		"INSERT INTO users (email, password_hash, full_name) VALUES ($1, $2, $3) RETURNING "+userCols,
		email, hash, fullName)
	return scanUser(row)
}

func AssignRole(ctx context.Context, q db.Querier, userID uuid.UUID, role string) error {
	_, err := q.Exec(ctx,
		"INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = $2 ON CONFLICT DO NOTHING",
		userID, role)
	return err
}

func FindByEmail(ctx context.Context, q db.Querier, email string) (User, error) {
	return scanUser(q.QueryRow(ctx, "SELECT "+userCols+" FROM users WHERE email = $1", email))
}

func FindByID(ctx context.Context, q db.Querier, id uuid.UUID) (User, error) {
	return scanUser(q.QueryRow(ctx, "SELECT "+userCols+" FROM users WHERE id = $1", id))
}

func Roles(ctx context.Context, q db.Querier, userID uuid.UUID) ([]string, error) {
	rows, err := q.Query(ctx,
		"SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.id WHERE ur.user_id = $1", userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return collectStrings(rows)
}

func Permissions(ctx context.Context, q db.Querier, userID uuid.UUID) ([]string, error) {
	rows, err := q.Query(ctx,
		`SELECT DISTINCT p.name FROM permissions p
		 JOIN role_permissions rp ON rp.permission_id = p.id
		 JOIN user_roles ur ON ur.role_id = rp.role_id
		 WHERE ur.user_id = $1`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return collectStrings(rows)
}

func collectStrings(rows interface {
	Next() bool
	Scan(...any) error
	Err() error
}) ([]string, error) {
	var out []string
	for rows.Next() {
		var s string
		if err := rows.Scan(&s); err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

func StoreRefresh(ctx context.Context, q db.Querier, jti, family, userID uuid.UUID, expiresAt time.Time) error {
	_, err := q.Exec(ctx,
		"INSERT INTO refresh_tokens (jti, family, user_id, expires_at) VALUES ($1, $2, $3, $4)",
		jti, family, userID, expiresAt)
	return err
}

type RefreshRecord struct {
	UserID  uuid.UUID
	Used    bool
	Revoked bool
}

func GetRefresh(ctx context.Context, q db.Querier, jti uuid.UUID) (RefreshRecord, error) {
	var r RefreshRecord
	err := q.QueryRow(ctx,
		"SELECT user_id, used, revoked FROM refresh_tokens WHERE jti = $1", jti).
		Scan(&r.UserID, &r.Used, &r.Revoked)
	return r, err
}

func MarkRefreshUsed(ctx context.Context, q db.Querier, jti uuid.UUID) error {
	_, err := q.Exec(ctx, "UPDATE refresh_tokens SET used = TRUE WHERE jti = $1", jti)
	return err
}

func RevokeFamily(ctx context.Context, q db.Querier, family uuid.UUID) error {
	_, err := q.Exec(ctx, "UPDATE refresh_tokens SET revoked = TRUE WHERE family = $1", family)
	return err
}
