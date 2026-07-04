// Package config loads typed application configuration from the environment
// (12-Factor). Missing required values fail fast at startup.
package config

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

type Config struct {
	Port             string
	DatabaseURL      string
	RedisURL         string
	JWTAccessSecret  string
	JWTRefreshSecret string
	AccessTTL        time.Duration
	RefreshTTL       time.Duration
	Issuer           string
	Audience         string
	CacheTTL         time.Duration
	LogFormat        string
}

func Load() (Config, error) {
	dbURL := os.Getenv("DATABASE_URL")
	if dbURL == "" {
		return Config{}, fmt.Errorf("DATABASE_URL is required")
	}
	accessSecret := os.Getenv("JWT_ACCESS_SECRET")
	if accessSecret == "" {
		return Config{}, fmt.Errorf("JWT_ACCESS_SECRET is required")
	}

	return Config{
		Port:             getenv("PORT", "8080"),
		DatabaseURL:      dbURL,
		RedisURL:         os.Getenv("REDIS_URL"), // optional
		JWTAccessSecret:  accessSecret,
		JWTRefreshSecret: getenv("JWT_REFRESH_SECRET", "refresh-secret"),
		AccessTTL:        time.Duration(getint("JWT_ACCESS_TTL_SECS", 900)) * time.Second,
		RefreshTTL:       time.Duration(getint("JWT_REFRESH_TTL_SECS", 2592000)) * time.Second,
		Issuer:           getenv("JWT_ISSUER", "ecommerce.go"),
		Audience:         getenv("JWT_AUDIENCE", "ecommerce.api"),
		CacheTTL:         time.Duration(getint("CACHE_TTL_SECS", 60)) * time.Second,
		LogFormat:        getenv("LOG_FORMAT", "json"),
	}, nil
}

func getenv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func getint(key string, def int) int {
	if v := os.Getenv(key); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			return n
		}
	}
	return def
}
