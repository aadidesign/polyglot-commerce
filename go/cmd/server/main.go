// Command server is the e-commerce backend (Go / chi).
//
// A clean-layered service: handler → repository → Postgres, with JWT auth +
// RBAC, Argon2id hashing, Redis caching, keyset pagination, RFC 7807 errors,
// Prometheus metrics, structured logging, graceful shutdown, and an embedded
// SQL migrator. Checkout is a single ACID transaction (no oversell).
package main

import (
	"context"
	"errors"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/redis/go-redis/v9"

	"ecommerce/internal/auth"
	"ecommerce/internal/config"
	"ecommerce/internal/db"
	"ecommerce/internal/server"
	"ecommerce/migrations"
)

func main() {
	// Container healthcheck: `server healthcheck` exits 0 if the port is open.
	if len(os.Args) > 1 && os.Args[1] == "healthcheck" {
		port := os.Getenv("PORT")
		if port == "" {
			port = "8080"
		}
		conn, err := net.DialTimeout("tcp", "127.0.0.1:"+port, 2*time.Second)
		if err != nil {
			os.Exit(1)
		}
		_ = conn.Close()
		return
	}

	if err := run(); err != nil {
		slog.Error("fatal", "error", err)
		os.Exit(1)
	}
}

func run() error {
	cfg, err := config.Load()
	if err != nil {
		return err
	}
	setupLogging(cfg.LogFormat)

	ctx := context.Background()
	pool, err := db.Connect(ctx, cfg.DatabaseURL)
	if err != nil {
		return err
	}
	defer pool.Close()

	if err := db.Migrate(ctx, pool, migrations.FS); err != nil {
		return err
	}
	slog.Info("migrations applied")

	var rdb *redis.Client
	if cfg.RedisURL != "" {
		opt, err := redis.ParseURL(cfg.RedisURL)
		if err != nil {
			slog.Warn("invalid REDIS_URL; caching disabled", "error", err)
		} else {
			rdb = redis.NewClient(opt)
			if err := rdb.Ping(ctx).Err(); err != nil {
				slog.Warn("Redis unavailable; caching disabled", "error", err)
				rdb = nil
			}
		}
	}

	mgr := auth.NewManager(cfg.JWTAccessSecret, cfg.JWTRefreshSecret,
		cfg.AccessTTL, cfg.RefreshTTL, cfg.Issuer, cfg.Audience)

	handler := server.New(cfg, pool, rdb, mgr)
	srv := &http.Server{
		Addr:              ":" + cfg.Port,
		Handler:           handler,
		ReadHeaderTimeout: 5 * time.Second,
	}

	// Graceful shutdown on SIGINT/SIGTERM.
	go func() {
		stop := make(chan os.Signal, 1)
		signal.Notify(stop, os.Interrupt, syscall.SIGTERM)
		<-stop
		slog.Info("shutting down")
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdownCtx)
	}()

	slog.Info("listening", "addr", srv.Addr)
	if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}

func setupLogging(format string) {
	var handler slog.Handler
	opts := &slog.HandlerOptions{Level: slog.LevelInfo}
	if format == "json" {
		handler = slog.NewJSONHandler(os.Stdout, opts)
	} else {
		handler = slog.NewTextHandler(os.Stdout, opts)
	}
	slog.SetDefault(slog.New(handler))
}
