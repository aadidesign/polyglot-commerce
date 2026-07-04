package auth

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestPasswordRoundTrip(t *testing.T) {
	hash, err := HashPassword("correct horse battery staple")
	if err != nil {
		t.Fatalf("hash: %v", err)
	}
	if !VerifyPassword("correct horse battery staple", hash) {
		t.Fatal("valid password rejected")
	}
	if VerifyPassword("wrong password", hash) {
		t.Fatal("invalid password accepted")
	}
}

func testManager() *Manager {
	return NewManager("access-secret-0123456789", "refresh-secret-0123456789",
		15*time.Minute, time.Hour, "test.iss", "test.aud")
}

func TestAccessTokenRoundTripAndRBAC(t *testing.T) {
	mgr := testManager()
	uid := uuid.New()
	issued, err := mgr.IssueAccess(uid, []string{"customer"}, []string{"order:write"})
	if err != nil {
		t.Fatalf("issue: %v", err)
	}
	claims, err := mgr.VerifyAccess(issued.Token)
	if err != nil {
		t.Fatalf("verify: %v", err)
	}
	if claims.Subject != uid.String() {
		t.Fatalf("subject mismatch: %s", claims.Subject)
	}
	if !claims.HasPermission("order:write") {
		t.Fatal("expected permission missing")
	}
	if claims.HasPermission("catalog:write") {
		t.Fatal("unexpected permission present")
	}
}

func TestAccessVerifierRejectsRefreshToken(t *testing.T) {
	mgr := testManager()
	issued, _ := mgr.IssueRefresh(uuid.New(), uuid.New())
	if _, err := mgr.VerifyAccess(issued.Token); err == nil {
		t.Fatal("refresh token must not verify as access token")
	}
}

func TestWildcardPermission(t *testing.T) {
	c := &AccessClaims{Permissions: []string{"*"}}
	if !c.HasPermission("anything:at:all") {
		t.Fatal("wildcard should grant any permission")
	}
}
