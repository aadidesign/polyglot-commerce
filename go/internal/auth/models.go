package auth

import (
	"net/mail"
	"time"

	"github.com/google/uuid"

	"ecommerce/internal/httpx"
)

type User struct {
	ID            uuid.UUID
	Email         string
	PasswordHash  string
	FullName      string
	EmailVerified bool
	Status        string
	CreatedAt     time.Time
}

type RegisterRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
	FullName string `json:"full_name"`
}

func (r RegisterRequest) Validate() error {
	if _, err := mail.ParseAddress(r.Email); err != nil {
		return httpx.BadRequest("invalid email")
	}
	if len(r.Password) < 8 || len(r.Password) > 128 {
		return httpx.BadRequest("password must be 8-128 characters")
	}
	if r.FullName == "" {
		return httpx.BadRequest("full_name is required")
	}
	return nil
}

type LoginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type RefreshRequest struct {
	RefreshToken string `json:"refresh_token"`
}

type LogoutRequest struct {
	RefreshToken string `json:"refresh_token"`
}

type UserResponse struct {
	ID            uuid.UUID `json:"id"`
	Email         string    `json:"email"`
	FullName      string    `json:"full_name"`
	EmailVerified bool      `json:"email_verified"`
	Status        string    `json:"status"`
	Roles         []string  `json:"roles"`
	CreatedAt     time.Time `json:"created_at"`
}

func newUserResponse(u User, roles []string) UserResponse {
	return UserResponse{
		ID:            u.ID,
		Email:         u.Email,
		FullName:      u.FullName,
		EmailVerified: u.EmailVerified,
		Status:        u.Status,
		Roles:         roles,
		CreatedAt:     u.CreatedAt,
	}
}

type TokenResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int64  `json:"expires_in"`
}

type RegisterResponse struct {
	User   UserResponse  `json:"user"`
	Tokens TokenResponse `json:"tokens"`
}
