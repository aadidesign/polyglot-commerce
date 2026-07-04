"""RFC 7807 problem responses and FastAPI exception handlers."""
from __future__ import annotations

import logging

from fastapi import Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from starlette.exceptions import HTTPException as StarletteHTTPException

log = logging.getLogger("ecommerce")


class APIError(Exception):
    def __init__(self, status: int, detail: str, title: str | None = None):
        self.status = status
        self.detail = detail
        self.title = title or _title(status)
        super().__init__(detail)


def _title(status: int) -> str:
    return {
        400: "Bad Request",
        401: "Unauthorized",
        403: "Forbidden",
        404: "Not Found",
        409: "Conflict",
        429: "Too Many Requests",
        500: "Internal Server Error",
    }.get(status, "Error")


def bad_request(detail: str) -> APIError:
    return APIError(400, detail)


def unauthorized() -> APIError:
    return APIError(401, "authentication required")


def forbidden() -> APIError:
    return APIError(403, "insufficient permissions")


def not_found(resource: str) -> APIError:
    return APIError(404, f"{resource} not found")


def conflict(detail: str) -> APIError:
    return APIError(409, detail)


def _problem(status: int, title: str, detail: str, errors=None) -> JSONResponse:
    body = {
        "type": f"https://errors.ecommerce.dev/{title.replace(' ', '-').lower()}",
        "title": title,
        "status": status,
        "detail": detail,
    }
    if errors is not None:
        body["errors"] = errors
    return JSONResponse(status_code=status, content=body, media_type="application/problem+json")


def register_handlers(app) -> None:
    @app.exception_handler(APIError)
    async def _api_error(_: Request, exc: APIError):
        return _problem(exc.status, exc.title, exc.detail)

    @app.exception_handler(RequestValidationError)
    async def _validation(_: Request, exc: RequestValidationError):
        return _problem(400, "Bad Request", "request validation failed", errors=exc.errors())

    @app.exception_handler(StarletteHTTPException)
    async def _http(_: Request, exc: StarletteHTTPException):
        return _problem(exc.status_code, _title(exc.status_code), str(exc.detail))

    @app.exception_handler(Exception)
    async def _unhandled(_: Request, exc: Exception):
        log.exception("unhandled error: %s", exc)
        return _problem(500, "Internal Server Error", "an internal error occurred")
