package auth

import (
	"errors"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

type AccessClaims struct {
	Roles       []string `json:"roles"`
	Permissions []string `json:"permissions"`
	Typ         string   `json:"typ"`
	jwt.RegisteredClaims
}

type RefreshClaims struct {
	Family string `json:"family"`
	Typ    string `json:"typ"`
	jwt.RegisteredClaims
}

func (c *AccessClaims) HasPermission(p string) bool {
	for _, perm := range c.Permissions {
		if perm == p || perm == "*" {
			return true
		}
	}
	return false
}

// Issued bundles a freshly minted token with the data the caller must persist.
type Issued struct {
	Token     string
	JTI       uuid.UUID
	ExpiresAt time.Time
}

type Manager struct {
	accessSecret  []byte
	refreshSecret []byte
	accessTTL     time.Duration
	refreshTTL    time.Duration
	issuer        string
	audience      string
}

func NewManager(accessSecret, refreshSecret string, accessTTL, refreshTTL time.Duration, issuer, audience string) *Manager {
	return &Manager{
		accessSecret:  []byte(accessSecret),
		refreshSecret: []byte(refreshSecret),
		accessTTL:     accessTTL,
		refreshTTL:    refreshTTL,
		issuer:        issuer,
		audience:      audience,
	}
}

func (m *Manager) IssueAccess(userID uuid.UUID, roles, perms []string) (Issued, error) {
	jti := uuid.New()
	now := time.Now()
	exp := now.Add(m.accessTTL)
	claims := AccessClaims{
		Roles:       roles,
		Permissions: perms,
		Typ:         "access",
		RegisteredClaims: jwt.RegisteredClaims{
			Subject:   userID.String(),
			Issuer:    m.issuer,
			Audience:  jwt.ClaimStrings{m.audience},
			ID:        jti.String(),
			IssuedAt:  jwt.NewNumericDate(now),
			ExpiresAt: jwt.NewNumericDate(exp),
		},
	}
	token, err := jwt.NewWithClaims(jwt.SigningMethodHS256, claims).SignedString(m.accessSecret)
	return Issued{Token: token, JTI: jti, ExpiresAt: exp}, err
}

func (m *Manager) IssueRefresh(userID, family uuid.UUID) (Issued, error) {
	jti := uuid.New()
	now := time.Now()
	exp := now.Add(m.refreshTTL)
	claims := RefreshClaims{
		Family: family.String(),
		Typ:    "refresh",
		RegisteredClaims: jwt.RegisteredClaims{
			Subject:   userID.String(),
			Issuer:    m.issuer,
			Audience:  jwt.ClaimStrings{m.audience},
			ID:        jti.String(),
			IssuedAt:  jwt.NewNumericDate(now),
			ExpiresAt: jwt.NewNumericDate(exp),
		},
	}
	token, err := jwt.NewWithClaims(jwt.SigningMethodHS256, claims).SignedString(m.refreshSecret)
	return Issued{Token: token, JTI: jti, ExpiresAt: exp}, err
}

func (m *Manager) parseOpts() []jwt.ParserOption {
	return []jwt.ParserOption{
		jwt.WithValidMethods([]string{"HS256"}),
		jwt.WithIssuer(m.issuer),
		jwt.WithAudience(m.audience),
		jwt.WithExpirationRequired(),
	}
}

func (m *Manager) VerifyAccess(token string) (*AccessClaims, error) {
	claims := &AccessClaims{}
	_, err := jwt.ParseWithClaims(token, claims, func(*jwt.Token) (any, error) {
		return m.accessSecret, nil
	}, m.parseOpts()...)
	if err != nil {
		return nil, err
	}
	if claims.Typ != "access" {
		return nil, errors.New("not an access token")
	}
	return claims, nil
}

func (m *Manager) VerifyRefresh(token string) (*RefreshClaims, error) {
	claims := &RefreshClaims{}
	_, err := jwt.ParseWithClaims(token, claims, func(*jwt.Token) (any, error) {
		return m.refreshSecret, nil
	}, m.parseOpts()...)
	if err != nil {
		return nil, err
	}
	if claims.Typ != "refresh" {
		return nil, errors.New("not a refresh token")
	}
	return claims, nil
}
