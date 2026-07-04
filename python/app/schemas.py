"""Pydantic request models (validation) used across routers."""
from __future__ import annotations

import uuid

from pydantic import BaseModel, EmailStr, Field


class RegisterRequest(BaseModel):
    email: EmailStr
    password: str = Field(min_length=8, max_length=128)
    full_name: str = Field(min_length=1, max_length=200)


class LoginRequest(BaseModel):
    email: EmailStr
    password: str = Field(min_length=1)


class RefreshRequest(BaseModel):
    refresh_token: str


class LogoutRequest(BaseModel):
    refresh_token: str


class CreateProductRequest(BaseModel):
    sku: str = Field(min_length=1, max_length=64)
    name: str = Field(min_length=1, max_length=200)
    slug: str = Field(min_length=1, max_length=200)
    description: str = ""
    price_cents: int = Field(ge=0)
    currency: str = "USD"
    stock_quantity: int = Field(default=0, ge=0)
    category_id: uuid.UUID | None = None


class UpdateProductRequest(BaseModel):
    name: str | None = Field(default=None, max_length=200)
    description: str | None = None
    price_cents: int | None = Field(default=None, ge=0)
    stock_quantity: int | None = Field(default=None, ge=0)
    category_id: uuid.UUID | None = None
    status: str | None = None


class CreateCategoryRequest(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    slug: str = Field(min_length=1, max_length=120)
    parent_id: uuid.UUID | None = None


class AddItemRequest(BaseModel):
    product_id: uuid.UUID
    quantity: int = Field(ge=1, le=1000)


class UpdateItemRequest(BaseModel):
    quantity: int = Field(ge=1, le=1000)
