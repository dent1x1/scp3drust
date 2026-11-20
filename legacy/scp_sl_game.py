#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SCP: Secret Laboratory — 2D prototype (Python + pygame)

Одиночный прототип в духе SCP:SL: вы — сотрудник класса D, сбежавший
из комплекса. Соберите карты доступа 1-3 уровня, откройте шлюз и
уберитесь отсюда. Главная угроза — SCP-173: он замирает, пока вы на
него смотрите, и убивает, стоит моргнуть.

Управление:
    WASD      — движение
    Мышь      — направление взгляда / фонаря
    Shift     — бег (тратит выносливость)
    F         — фонарь
    Esc       — пауза
    R         — заново (после смерти / победы)
"""

import os
import sys
import math
import random
from collections import deque

# --- скриншот-режим (без окна) -------------------------------------------
ARGS = sys.argv[1:]
SHOT_PATH = None
MENU_SHOT = False
if "--screenshot" in ARGS:
    SHOT_PATH = ARGS[ARGS.index("--screenshot") + 1]
    os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
if "--menu-shot" in ARGS:
    MENU_SHOT = True
    SHOT_PATH = ARGS[ARGS.index("--menu-shot") + 1]
    os.environ.setdefault("SDL_VIDEODRIVER", "dummy")

import pygame
from pygame.math import Vector2

# --- константы ------------------------------------------------------------
TS = 64                      # размер тайла
SCREEN_W, SCREEN_H = 1280, 720
FPS = 60
PLAYER_R = 15
STATUE_R = 17
PLAY_SPEED = 218
SPRINT_MULT = 1.5
STATUE_SPEED = 252

# цвета
C_BG = (11, 15, 20)
C_FLOOR = (26, 33, 42)
C_FLOOR_2 = (31, 39, 50)
C_WALL = (38, 47, 58)
C_WALL_TOP = (73, 87, 103)
C_WALL_DARK = (21, 27, 34)
C_TEXT = (199, 213, 224)
C_MUTED = (124, 140, 155)
C_ACCENT = (102, 192, 244)
C_GREEN = (164, 208, 7)
C_RED = (224, 86, 86)
C_YELLOW = (255, 207, 92)

CARD_COLORS = {1: (102, 192, 244), 2: (255, 207, 92), 3: (224, 86, 86)}


# --- утилиты --------------------------------------------------------------
_fonts = {}


def font(size, bold=False, mono=False):
    key = (size, bold, mono)
    if key not in _fonts:
        name = "consolas" if mono else "segoeui"
        try:
            f = pygame.font.SysFont(name, size, bold=bold)
        except Exception:
            f = pygame.font.Font(None, size)
        _fonts[key] = f
    return _fonts[key]


def text(surf, s, pos, size=18, color=C_TEXT, bold=False, center=False,
         alpha=255, mono=False, shadow=False):
    img = font(size, bold, mono).render(s, True, color)
    if alpha < 255:
        img.set_alpha(alpha)
    r = img.get_rect()
    if center:
        r.center = pos
    else:
        r.topleft = pos
    if shadow:
        sh = font(size, bold, mono).render(s, True, (0, 0, 0))
        sh.set_alpha(int(alpha * 0.7))
        surf.blit(sh, (r.x + 2, r.y + 2))
    surf.blit(img, r)
    return r


def panel(surf, rect, color=(10, 16, 24), alpha=205, border=(42, 63, 85)):
    s = pygame.Surface(rect.size, pygame.SRCALPHA)
    pygame.draw.rect(s, (*color, alpha), s.get_rect(), border_radius=4)
    pygame.draw.rect(s, (*border, 220), s.get_rect(), 1, border_radius=4)
    surf.blit(s, rect.topleft)


def ang_diff(a, b):
    d = (a - b + math.pi) % (math.tau) - math.pi
    return abs(d)


def fmt_time(t):
    m = int(t) // 60
    s = int(t) % 60
    return f"{m:02d}:{s:02d}"


# --- карта ----------------------------------------------------------------
class Facility:
    W, H = 40, 28

    def __init__(self):
        self.tiles = [["#"] * self.W for _ in range(self.H)]
        self.rooms = [
            (2, 2, 9, 7, "ЗОНА СОДЕРЖАНИЯ"),
            (14, 2, 10, 6, "ОФИСЫ"),
            (28, 2, 9, 8, "ЛАБОРАТОРИЯ"),
            (2, 13, 10, 7, "КПП / ГАРАЖ"),
            (15, 11, 8, 7, "СТОЛОВАЯ"),
            (28, 14, 9, 9, "РЕАКТОР"),
            (15, 21, 8, 5, "ШЛЮЗ"),
        ]
        for x, y, w, h, _ in self.rooms:
            self.carve(x, y, x + w - 1, y + h - 1)

        # коридоры (1 тайл шириной)
        self.carve(10, 5, 14, 5)     # содержание -> офисы
        self.carve(23, 5, 28, 5)     # офисы -> лаборатория
        self.carve(6, 8, 6, 13)      # содержание -> КПП
        self.carve(11, 16, 15, 16)   # КПП -> столовая
        self.carve(18, 7, 18, 11)    # офисы -> столовая
        self.carve(32, 9, 32, 14)    # лаборатория -> реактор
        self.carve(22, 14, 28, 14)   # столовая -> реактор
        self.carve(18, 17, 18, 21)   # столовая -> шлюз

        # двери: (tx, ty, уровень)
        self.door_defs = [
            (12, 5, 1, "ЗОНА СОДЕРЖАНИЯ — ОФИСЫ"),
            (25, 5, 2, "ОФИСЫ — ЛАБОРАТОРИЯ"),
            (32, 11, 2, "ЛАБОРАТОРИЯ — РЕАКТОР"),
            (18, 19, 3, "ШЛЮЗ — ВЫХОД"),
        ]

        # выход
        self.exit_rect = pygame.Rect(17 * TS, 22 * TS, 4 * TS, 2 * TS)
        self.spawn = Vector2(6.5 * TS, 16.5 * TS)
        self.statue_spawn = Vector2(6.5 * TS, 4.3 * TS)
        self.card_spots = [(9.4, 3.4, 1), (21.5, 3.4, 2), (34.4, 7.3, 3)]

        rng = random.Random(173)
        self.decals = []
        for _ in range(220):
            tx = rng.randrange(self.W)
            ty = rng.randrange(self.H)
            if self.tiles[ty][tx] == ".":
                self.decals.append((tx * TS + rng.randint(6, 58),
                                    ty * TS + rng.randint(6, 58),
                                    rng.choice([1, 2, 2, 3]),
                                    rng.randint(18, 40)))
        # кровь у статуи
        brng = random.Random(9)
        self.blood = [(6.2 + brng.uniform(-1.6, 1.8), 4.6 + brng.uniform(-1.4, 1.6),
                       brng.randint(6, 15)) for _ in range(9)]
        self.build_static()

    def carve(self, x0, y0, x1, y1):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.tiles[y][x] = "."

    def room_at(self, tx, ty):
        for x, y, w, h, name in self.rooms:
            if x <= tx < x + w and y <= ty < y + h:
                return name
        return "КОРИДОР"

    def build_static(self):
        self.map_w = self.W * TS
        self.map_h = self.H * TS
        surf = pygame.Surface((self.map_w, self.map_h)).convert()
        surf.fill(C_WALL_DARK)
        rng = random.Random(4)

        # стены
        for ty in range(self.H):
            for tx in range(self.W):
                if self.tiles[ty][tx] != "#":
                    continue
                r = pygame.Rect(tx * TS, ty * TS, TS, TS)
                v = rng.randint(-5, 5)
                surf.fill((C_WALL[0] + v, C_WALL[1] + v, C_WALL[2] + v), r)
                # нижняя грань, если снизу пол — «объём»
                if ty + 1 < self.H and self.tiles[ty + 1][tx] == ".":
                    pygame.draw.rect(surf, C_WALL_TOP, (r.x, r.bottom - 5, TS, 5))
                    pygame.draw.line(surf, (12, 16, 20), r.bottomleft, r.bottomright, 2)
                if tx + 1 < self.W and self.tiles[ty][tx + 1] == ".":
                    pygame.draw.line(surf, (24, 30, 38), (r.right - 2, r.y), (r.right - 2, r.bottom), 3)
                pygame.draw.rect(surf, (28, 35, 44), r, 1)

        # пол
        for ty in range(self.H):
            for tx in range(self.W):
                if self.tiles[ty][tx] != ".":
                    continue
                r = pygame.Rect(tx * TS, ty * TS, TS, TS)
                c = C_FLOOR if (tx + ty) % 2 else C_FLOOR_2
                surf.fill(c, r)
                pygame.draw.rect(surf, (21, 28, 36), r, 1)
                # блики на полу
                if rng.random() < 0.3:
                    pygame.draw.line(surf, (40, 51, 64),
                                     (r.x + 8, r.y + rng.randint(8, 56)),
                                     (r.x + rng.randint(20, 56), r.y + rng.randint(8, 56)), 1)

        # трещины / мусор
        for x, y, w, a in self.decals:
            pygame.draw.line(surf, (18, 24, 31), (x, y), (x + 10, y + 5), w)

        # кровь
        for bx, by, rad in self.blood:
            s = pygame.Surface((rad * 2, rad * 2), pygame.SRCALPHA)
            pygame.draw.ellipse(s, (92, 12, 12, 70), s.get_rect())
            surf.blit(s, (bx * TS - rad, by * TS - rad))

        # разметка перед дверями
        for tx, ty, lvl, _ in self.door_defs:
            col = CARD_COLORS[lvl]
            r = pygame.Rect(tx * TS, ty * TS, TS, TS)
            for i in range(0, TS, 16):
                pygame.draw.polygon(surf, (58, 52, 24),
                                    [(r.x + i, r.bottom), (r.x + i + 8, r.bottom),
                                     (r.x + i + 22, r.y), (r.x + i + 14, r.y)])
            pygame.draw.rect(surf, col, (r.x + 3, r.y + 3, TS - 6, 4), border_radius=2)

        # названия секторов на полу
        for x, y, w, h, name in self.rooms:
            cx = int((x + w / 2) * TS)
            cy = int((y + h / 2) * TS)
            img = font(22, bold=True).render(name, True, (72, 88, 104))
            img.set_alpha(70)
            surf.blit(img, img.get_rect(center=(cx, cy)))

        # выход
        ex = self.exit_rect
        s = pygame.Surface(ex.size, pygame.SRCALPHA)
        pygame.draw.rect(s, (30, 90, 46, 120), s.get_rect(), border_radius=6)
        surf.blit(s, ex.topleft)

        self.static = surf


# --- объекты --------------------------------------------------------------
class Door:
    def __init__(self, tx, ty, need, label):
        self.tx, self.ty = tx, ty
        self.need = need
        self.label = label
        self.open = False
        self.msg_t = 0.0

    @property
    def rect(self):
        return pygame.Rect(self.tx * TS, self.ty * TS, TS, TS)

    @property
    def center(self):
        return Vector2(self.tx * TS + TS / 2, self.ty * TS + TS / 2)


class Player:
    def __init__(self, pos):
        self.pos = Vector2(pos)
        self.angle = -math.pi / 2
        self.stamina = 100.0
        self.battery = 100.0
        self.flash = True
        self.cards = set()
        self.blink_t = 6.5
        self.blink_interval = 6.5
        self.blinking = 0.0
        self.sector = ""


class Statue:
    def __init__(self, pos):
        self.pos = Vector2(pos)
        self.active = False
        self.frozen = True
        self.path = []
        self.path_t = 0.0
        self.face = 0.0


# --- игра -----------------------------------------------------------------
class Game:
    def __init__(self):
        pygame.init()
        flags = 0 if SHOT_PATH else (pygame.SCALED | pygame.RESIZABLE)
        self.screen = pygame.display.set_mode((SCREEN_W, SCREEN_H), flags, vsync=0)
        pygame.display.set_caption("SCP: Secret Laboratory — 2D prototype")
        self.clock = pygame.time.Clock()
        self.fac = Facility()
        self.doors = [Door(*d) for d in self.fac.door_defs]
        self.door_at = {(d.tx, d.ty): d for d in self.doors}
        self.cards = [{"x": x * TS, "y": y * TS, "lvl": lvl, "taken": False}
                      for x, y, lvl in self.fac.card_spots]
        self.toasts = []
        self.state = "menu"
        self.t = 0.0
        self.run_time = 0.0
        self.camera = Vector2()
        self.shake = 0.0
        self.mouse = (SCREEN_W // 2, SCREEN_H // 2)
        self.start_x = 0
        self.light = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        self.vignette = self.build_vignette()
        self.reset()

    # ---------- настройка ----------
    def reset(self):
        self.player = Player(self.fac.spawn)
        self.statue = Statue(self.fac.statue_spawn)
        for d in self.doors:
            d.open = False
            d.msg_t = 0
        for c in self.cards:
            c["taken"] = False
        self.toasts.clear()
        self.run_time = 0.0
        self.shake = 0.0
        self.threat = 0.0
        self.death_t = 0.0
        self.win_t = 0.0
        self.light_warned = False
        self.sector_t = 0.0
        self.camera = self.camera_target()

    def start(self):
        self.reset()
        self.state = "play"

    def camera_target(self):
        x = self.player.pos.x - SCREEN_W / 2
        y = self.player.pos.y - SCREEN_H / 2
        x = max(0, min(x, self.fac.map_w - SCREEN_W))
        y = max(0, min(y, self.fac.map_h - SCREEN_H))
        return Vector2(x, y)

    @staticmethod
    def build_vignette():
        s = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        cx, cy = SCREEN_W / 2, SCREEN_H / 2
        maxd = math.hypot(cx, cy)
        step = 10
        for y in range(0, SCREEN_H, step):
            for x in range(0, SCREEN_W, step):
                d = math.hypot(x - cx, y - cy) / maxd
                a = int(max(0.0, (d - 0.45) / 0.55) ** 1.7 * 170)
                if a > 0:
                    pygame.draw.rect(s, (0, 0, 0, a), (x, y, step, step))
        return s

    # ---------- геометрия ----------
    def solid_tile(self, tx, ty):
        if tx < 0 or ty < 0 or tx >= Facility.W or ty >= Facility.H:
            return True
        if self.fac.tiles[ty][tx] == "#":
            return True
        d = self.door_at.get((tx, ty))
        return bool(d and not d.open)

    def solid_at(self, x, y):
        return self.solid_tile(int(x // TS), int(y // TS))

    def circle_solid(self, x, y, r):
        x0 = int((x - r) // TS)
        x1 = int((x + r) // TS)
        y0 = int((y - r) // TS)
        y1 = int((y + r) // TS)
        for ty in range(y0, y1 + 1):
            for tx in range(x0, x1 + 1):
                if not self.solid_tile(tx, ty):
                    continue
                nx = max(tx * TS, min(x, tx * TS + TS))
                ny = max(ty * TS, min(y, ty * TS + TS))
                if (x - nx) ** 2 + (y - ny) ** 2 < r * r:
                    return True
        return False

    def los(self, a, b, step=7):
        d = a.distance_to(b)
        if d < 1:
            return True
        n = int(d / step) + 1
        for i in range(1, n):
            p = a.lerp(b, i / n)
            if self.solid_at(p.x, p.y):
                return False
        return True

    def cast(self, ang, maxd):
        px, py = self.player.pos
        d = 0.0
        while d < maxd:
            d += 5
            if self.solid_at(px + math.cos(ang) * d, py + math.sin(ang) * d):
                return max(0.0, d - 5)
        return maxd

    def vision(self):
        if self.player.flash and self.player.battery > 0:
            return 450, 0.40
        return 195, 0.95

    # ---------- обновление ----------
    def update(self, dt):
        self.t += dt
        self.mouse = pygame.mouse.get_pos()
        for toast in self.toasts:
            toast["t"] -= dt
        self.toasts = [t for t in self.toasts if t["t"] > 0]

        if self.state != "play":
            if self.state == "dead":
                self.death_t += dt
            if self.state == "win":
                self.win_t += dt
            self.camera += (self.camera_target() - self.camera) * min(1, dt * 6)
            return

        self.run_time += dt
        p = self.player

        # взгляд
        world_mouse = Vector2(self.mouse) + self.camera
        p.angle = math.atan2(world_mouse.y - p.pos.y, world_mouse.x - p.pos.x)

        # движение
        keys = pygame.key.get_pressed()
        mv = Vector2(int(keys[pygame.K_d]) - int(keys[pygame.K_a]),
                     int(keys[pygame.K_s]) - int(keys[pygame.K_w]))
        if mv.length_squared() > 0:
            mv = mv.normalize()
        sprinting = keys[pygame.K_LSHIFT] and p.stamina > 1 and mv.length_squared() > 0
        speed = PLAY_SPEED * (SPRINT_MULT if sprinting else 1.0)
        if sprinting:
            p.stamina = max(0.0, p.stamina - 26 * dt)
        else:
            p.stamina = min(100.0, p.stamina + 15 * dt)
        if mv.length_squared() > 0:
            delta = mv * speed * dt
            nx = p.pos.x + delta.x
            if not self.circle_solid(nx, p.pos.y, PLAYER_R):
                p.pos.x = nx
            ny = p.pos.y + delta.y
            if not self.circle_solid(p.pos.x, ny, PLAYER_R):
                p.pos.y = ny

        # фонарь
        if p.flash and p.battery > 0:
            p.battery = max(0.0, p.battery - 2.0 * dt)
            if p.battery <= 0 and not self.light_warned:
                self.light_warned = True
                self.toast("ФОНАРЬ РАЗРЯЖЕН — останься в темноте", C_RED)
        elif not p.flash:
            p.battery = min(100.0, p.battery + 0.6 * dt)

        # моргание
        p.blink_t -= dt
        if p.blink_t <= 0:
            p.blinking = 0.22
            p.blink_interval = random.uniform(5.5, 8.0)
            p.blink_t = p.blink_interval
        if p.blinking > 0:
            p.blinking = max(0.0, p.blinking - dt)

        # двери и карты
        self.interact_doors()
        self.pickups()

        # статуя
        self.update_statue(dt)
        if self.state != "play":
            self.camera += (self.camera_target() - self.camera) * min(1, dt * 9)
            return

        # сектор
        sec = self.fac.room_at(int(p.pos.x // TS), int(p.pos.y // TS))
        if sec != p.sector:
            p.sector = sec
            self.sector_t = 2.2
            self.toast(sec, C_MUTED)
        self.sector_t = max(0.0, self.sector_t - dt)

        # выход
        if self.fac.exit_rect.collidepoint(p.pos.x, p.pos.y):
            gate = self.door_at.get((18, 19))
            if gate and gate.open:
                self.state = "win"
                self.win_t = 0.0
                return

        # тревога
        s = self.statue
        if s.active and not s.frozen:
            dist = p.pos.distance_to(s.pos)
            target = 1.0 if dist < 480 else 0.3
            self.threat += (target - self.threat) * min(1, dt * 5)
        else:
            self.threat += (0.0 - self.threat) * min(1, dt * 3)

        self.camera += (self.camera_target() - self.camera) * min(1, dt * 9)

    def update_statue(self, dt):
        s = self.statue
        p = self.player
        if not s.active:
            s.frozen = True
            return
        dist = p.pos.distance_to(s.pos)
        r, half = self.vision()
        visible = False
        if p.blinking <= 0 and dist < r + 30:
            to_s = math.atan2(s.pos.y - p.pos.y, s.pos.x - p.pos.x)
            if ang_diff(to_s, p.angle) < half + 0.02 and self.los(p.pos, s.pos):
                visible = True
        close_los = p.blinking <= 0 and dist < 100 and self.los(p.pos, s.pos)
        s.frozen = visible or close_los
        s.face = math.atan2(p.pos.y - s.pos.y, p.pos.x - s.pos.x)

        # смерть
        if dist < PLAYER_R + STATUE_R + 22 and not s.frozen:
            self.state = "dead"
            self.death_t = 0.0
            self.shake = 18
            return

        if s.frozen:
            return

        speed = STATUE_SPEED
        if p.blinking > 0:
            speed *= 1.35
        s.path_t -= dt
        if s.path_t <= 0:
            s.path_t = 0.45
            start = (int(s.pos.x // TS), int(s.pos.y // TS))
            goal = (int(p.pos.x // TS), int(p.pos.y // TS))
            s.path = self.bfs(start, goal)
        # идём по пути
        while s.path and Vector2(s.path[0][0] * TS + TS / 2,
                                 s.path[0][1] * TS + TS / 2).distance_to(s.pos) < 14:
            s.path.pop(0)
        if s.path:
            tgt = Vector2(s.path[0][0] * TS + TS / 2, s.path[0][1] * TS + TS / 2)
        else:
            tgt = Vector2(p.pos)
        step = tgt - s.pos
        if step.length_squared() > 1:
            step = step.normalize() * speed * dt
            nx = s.pos.x + step.x
            if not self.circle_solid(nx, s.pos.y, STATUE_R):
                s.pos.x = nx
            ny = s.pos.y + step.y
            if not self.circle_solid(s.pos.x, ny, STATUE_R):
                s.pos.y = ny

    def bfs(self, start, goal):
        if start == goal:
            return [goal]
        if not self.walkable(*goal):
            return []
        q = deque([start])
        prev = {start: None}
        while q:
            cur = q.popleft()
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                n = (cur[0] + dx, cur[1] + dy)
                if n in prev or not self.walkable(*n):
                    continue
                prev[n] = cur
                if n == goal:
                    path = [n]
                    while prev[n] is not None:
                        n = prev[n]
                        path.append(n)
                    return path[::-1]
                q.append(n)
        return []

    def walkable(self, tx, ty):
        if tx < 0 or ty < 0 or tx >= Facility.W or ty >= Facility.H:
            return False
        return not self.solid_tile(tx, ty)

    def interact_doors(self):
        p = self.player
        tx, ty = int(p.pos.x // TS), int(p.pos.y // TS)
        for door in self.doors:
            if door.open:
                continue
            if abs(door.tx - tx) <= 1 and abs(door.ty - ty) <= 1:
                if door.center.distance_to(p.pos) < 58:
                    if door.need in p.cards or 3 in p.cards or (p.cards and max(p.cards) >= door.need):
                        door.open = True
                        self.toast(f"ДВЕРЬ ОТКРЫТА: {door.label}", C_GREEN)
                    elif self.t > door.msg_t:
                        door.msg_t = self.t + 1.6
                        self.toast(f"ТРЕБУЕТСЯ КАРТА УРОВНЯ {door.need}", C_RED)

    def pickups(self):
        p = self.player
        for c in self.cards:
            if c["taken"]:
                continue
            if p.pos.distance_to(Vector2(c["x"], c["y"])) < 42:
                c["taken"] = True
                p.cards.add(c["lvl"])
                names = {1: "ЗОНА СОДЕРЖАНИЯ", 2: "ОФИСЫ", 3: "ЛАБОРАТОРИЯ"}
                self.toast(f"КАРТА УРОВНЯ {c['lvl']} ПОЛУЧЕНА ({names[c['lvl']]})",
                           CARD_COLORS[c["lvl"]])
                if c["lvl"] == 1 and not self.statue.active:
                    self.statue.active = True
                    self.statue.path_t = 0
                    self.toast("ВНИМАНИЕ: SCP-173 ПОКИНУЛ КАМЕРУ", C_RED)

    def toast(self, msg, color=C_TEXT):
        self.toasts.append({"text": msg, "color": color, "t": 2.6})
        if len(self.toasts) > 4:
            self.toasts.pop(0)

    # ---------- отрисовка ----------
    def draw(self):
        s = self.screen
        s.fill(C_BG)
        if self.state == "menu":
            self.draw_menu()
            return

        cam = self.camera
        if self.shake > 0.5:
            cam = cam + Vector2(random.uniform(-self.shake, self.shake),
                                random.uniform(-self.shake, self.shake))
            self.shake *= 0.88
        camx, camy = int(cam.x), int(cam.y)

        s.blit(self.fac.static, (-camx, -camy))

        # выход
        pulse = (math.sin(self.t * 3) + 1) / 2
        ex = self.fac.exit_rect.move(-camx, -camy)
        glow = pygame.Surface(ex.size, pygame.SRCALPHA)
        pygame.draw.rect(glow, (90, 255, 120, int(28 + 40 * pulse)), glow.get_rect(), border_radius=8)
        pygame.draw.rect(glow, (120, 255, 140, int(90 + 60 * pulse)), glow.get_rect(), 2, border_radius=8)
        s.blit(glow, ex.topleft)
        text(s, "ВЫХОД", ex.center, 22, (170, 255, 180), bold=True, center=True,
             alpha=int(150 + 80 * pulse))

        # карты
        for c in self.cards:
            if c["taken"]:
                continue
            self.draw_card(c["x"] - camx, c["y"] - camy, c["lvl"])

        # двери
        for d in self.doors:
            self.draw_door(d, camx, camy)

        # статуя
        self.draw_statue(camx, camy)

        # свет
        self.compute_light()
        s.blit(self.light, (0, 0))

        # глаза статуи видны в темноте
        self.draw_statue_eyes(camx, camy)

        # игрок
        self.draw_player(camx, camy)

        # прицел
        mx, my = self.mouse
        pygame.draw.circle(s, (220, 235, 245), (mx, my), 2)
        pygame.draw.circle(s, (220, 235, 245, 90), (mx, my), 9, 1)

        s.blit(self.vignette, (0, 0))

        # тревожный красный эффект
        if self.threat > 0.02:
            a = int(self.threat * (34 + 22 * math.sin(self.t * 7)))
            red = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
            pygame.draw.rect(red, (150, 0, 0, max(0, a)), red.get_rect(), 26)
            s.blit(red, (0, 0))

        self.draw_hud()

        if self.state == "pause":
            self.draw_pause()
        elif self.state == "dead":
            self.draw_dead()
        elif self.state == "win":
            self.draw_win()

    def draw_card(self, x, y, lvl):
        col = CARD_COLORS[lvl]
        pulse = (math.sin(self.t * 4 + lvl) + 1) / 2
        g = pygame.Surface((70, 70), pygame.SRCALPHA)
        pygame.draw.circle(g, (*col, int(30 + 40 * pulse)), (35, 35), int(22 + 6 * pulse))
        self.screen.blit(g, (x - 35, y - 35))
        r = pygame.Rect(0, 0, 28, 18)
        r.center = (int(x), int(y))
        pygame.draw.rect(self.screen, (18, 22, 28), r.inflate(4, 4), border_radius=3)
        pygame.draw.rect(self.screen, col, r, border_radius=3)
        text(self.screen, str(lvl), r.center, 13, (10, 14, 18), bold=True, center=True)

    def draw_door(self, d, camx, camy):
        r = d.rect.move(-camx, -camy)
        col = CARD_COLORS[d.need]
        if d.open:
            pygame.draw.rect(self.screen, (16, 22, 28), r)
            pygame.draw.rect(self.screen, col, (r.x, r.y, 7, r.h))
            pygame.draw.rect(self.screen, col, (r.right - 7, r.y, 7, r.h))
            pygame.draw.line(self.screen, (60, 70, 80), (r.x, r.bottom - 1), (r.right, r.bottom - 1))
        else:
            pygame.draw.rect(self.screen, (48, 58, 70), r)
            for i in range(4, TS, 12):
                pygame.draw.line(self.screen, (57, 68, 82), (r.x + 2, r.y + i), (r.right - 2, r.y + i), 2)
            pygame.draw.rect(self.screen, col, r, 3)
            pygame.draw.circle(self.screen, (16, 20, 26), r.center, 15)
            pygame.draw.circle(self.screen, col, r.center, 15, 2)
            text(self.screen, str(d.need), r.center, 15, col, bold=True, center=True)

    def draw_statue(self, camx, camy):
        s = self.statue
        x, y = int(s.pos.x - camx), int(s.pos.y - camy)
        if not (-100 < x < SCREEN_W + 100 and -100 < y < SCREEN_H + 100):
            return
        # тень
        sh = pygame.Surface((72, 38), pygame.SRCALPHA)
        pygame.draw.ellipse(sh, (0, 0, 0, 100), sh.get_rect())
        self.screen.blit(sh, (x - 36, y - 6))
        # тело (вид сверху)
        ang = s.face
        fx, fy = math.cos(ang), math.sin(ang)
        px, py = -fy, fx
        pygame.draw.ellipse(self.screen, (86, 87, 80),
                            pygame.Rect(x - 24, y - 17, 48, 34))
        # плечи и руки
        for side in (-1, 1):
            ax = x + px * 20 * side + fx * 2
            ay = y + py * 20 * side + fy * 2
            pygame.draw.circle(self.screen, (128, 129, 118), (int(ax), int(ay)), 9)
            hx = x + px * 24 * side + fx * 9
            hy = y + py * 24 * side + fy * 9
            pygame.draw.circle(self.screen, (142, 143, 130), (int(hx), int(hy)), 6)
        # корпус
        pygame.draw.ellipse(self.screen, (138, 139, 126),
                            pygame.Rect(x - 17, y - 13, 34, 26))
        # голова, смещена вперёд
        hx = int(x + fx * 9)
        hy = int(y + fy * 9)
        pygame.draw.circle(self.screen, (158, 159, 144), (hx, hy), 11)
        pygame.draw.circle(self.screen, (176, 177, 160), (hx - 3, hy - 3), 5)
        # трещины
        pygame.draw.line(self.screen, (96, 97, 88), (x - 9, y + 3), (x + 7, y + 9), 1)
        pygame.draw.line(self.screen, (112, 113, 102), (x + 2, y - 8), (x + 12, y + 2), 1)

    def draw_statue_eyes(self, camx, camy):
        s = self.statue
        if not s.active:
            return
        p = self.player
        if not self.los(p.pos, s.pos):
            return
        x, y = int(s.pos.x - camx), int(s.pos.y - camy)
        ang = s.face
        px, py = -math.sin(ang), math.cos(ang)
        fx, fy = math.cos(ang), math.sin(ang)
        for side in (-1, 1):
            ex = int(x + fx * 17 + px * 6 * side)
            ey = int(y + fy * 17 + py * 6 * side)
            pygame.draw.circle(self.screen, (190, 40, 30), (ex, ey), 3)
            pygame.draw.circle(self.screen, (255, 170, 150), (ex, ey), 1)
        if s.frozen:
            text(self.screen, "SCP-173", (x, y - 48), 14, (220, 120, 110),
                 bold=True, center=True, alpha=190, shadow=True)

    def draw_player(self, camx, camy):
        p = self.player
        x, y = int(p.pos.x - camx), int(p.pos.y - camy)
        sh = pygame.Surface((44, 30), pygame.SRCALPHA)
        pygame.draw.ellipse(sh, (0, 0, 0, 120), sh.get_rect())
        self.screen.blit(sh, (x - 22, y - 2))
        pygame.draw.circle(self.screen, (34, 42, 52), (x, y), PLAYER_R + 2)
        pygame.draw.circle(self.screen, (208, 143, 96), (x, y), 9)
        pygame.draw.circle(self.screen, (44, 58, 72), (x, y), PLAYER_R)
        pygame.draw.circle(self.screen, (224, 168, 120), (x, y), 7)
        # направление
        fx = x + math.cos(p.angle) * 15
        fy = y + math.sin(p.angle) * 15
        pygame.draw.line(self.screen, (102, 192, 244), (x, y), (fx, fy), 3)

    def compute_light(self):
        m = self.light
        m.fill((0, 0, 0, 220))
        p = self.player
        if p.blinking > 0:
            m.fill((0, 0, 0, 243))
            return
        cam = self.camera
        sx, sy = int(p.pos.x - cam.x), int(p.pos.y - cam.y)
        # мягкий ореол вокруг игрока
        for i in range(8, 0, -1):
            rad = int(150 * i / 8)
            a = int(220 * (i / 8) ** 2.2)
            pygame.draw.circle(m, (0, 0, 0, a), (sx, sy), rad)
        r, half = self.vision()
        pts = self.cone_points(r, half, sx, sy)
        if len(pts) > 2:
            # мягкая кромка
            edge = self.cone_points(r * 1.04, half + 0.13, sx, sy)
            if len(edge) > 2:
                pygame.draw.polygon(m, (0, 0, 0, 130), edge)
            pygame.draw.polygon(m, (0, 0, 0, 0), pts)

    def cone_points(self, r, half, sx, sy):
        p = self.player
        pts = [(sx, sy)]
        n = 34
        for i in range(n + 1):
            a = p.angle - half + (2 * half) * i / n
            d = self.cast(a, r)
            pts.append((int(sx + math.cos(a) * d), int(sy + math.sin(a) * d)))
        return pts

    # ---------- интерфейс ----------
    def draw_hud(self):
        s = self.screen
        p = self.player

        # задача
        if 1 not in p.cards:
            obj = "Найти карту уровня 1 в Зоне содержания"
            sub = "Сектор: ЗОНА СОДЕРЖАНИЯ — на севере"
        elif 2 not in p.cards:
            obj = "Найти карту уровня 2 в Офисах"
            sub = "Дверь на восток от Зоны содержания"
        elif 3 not in p.cards:
            obj = "Найти карту уровня 3 в Лаборатории"
            sub = "Дверь из Офисов на восток"
        else:
            obj = "Добраться до шлюза и покинуть комплекс"
            sub = "Шлюз — юг Столовой"
        panel(s, pygame.Rect(18, 16, 440, 74))
        text(s, "ЗАДАЧА", (32, 23), 11, C_MUTED, bold=True)
        text(s, obj, (32, 37), 16, (235, 244, 250))
        text(s, sub, (32, 62), 12, C_MUTED)

        # таймер
        panel(s, pygame.Rect(SCREEN_W - 208, 16, 190, 56))
        text(s, "ВРЕМЯ В КОМПЛЕКСЕ", (SCREEN_W - 192, 24), 10, C_MUTED, bold=True)
        text(s, fmt_time(self.run_time), (SCREEN_W - 192, 39), 22, (255, 255, 255), bold=True, mono=True)
        text(s, "SCP:SL // 2D", (SCREEN_W - 192, 64), 9, (90, 104, 116))

        # сектор
        panel(s, pygame.Rect(18, SCREEN_H - 116, 320, 46))
        text(s, "СЕКТОР", (32, SCREEN_H - 108), 10, C_MUTED, bold=True)
        text(s, p.sector or "—", (100, SCREEN_H - 110), 15, (235, 244, 250))

        # карты
        bx = 18
        for lvl in (1, 2, 3):
            r = pygame.Rect(bx, SCREEN_H - 62, 34, 24)
            owned = lvl in p.cards
            col = CARD_COLORS[lvl]
            pygame.draw.rect(s, (12, 17, 23), r, border_radius=3)
            pygame.draw.rect(s, col if owned else (60, 70, 82), r, 2, border_radius=3)
            text(s, str(lvl), r.center, 12, col if owned else (80, 92, 104), bold=True, center=True)
            bx += 42
        text(s, "УРОВЕНЬ ДОСТУПА", (bx + 2, SCREEN_H - 58), 10, C_MUTED)

        # полосы справа
        x0 = SCREEN_W - 250
        y0 = SCREEN_H - 108
        self.hud_bar(x0, y0, "ВЫНОСЛИВОСТЬ", p.stamina / 100, (164, 208, 7), f"{int(p.stamina)}%")
        self.hud_bar(x0, y0 + 30, "ФОНАРЬ", p.battery / 100, (255, 207, 92), f"{int(p.battery)}%")
        blink = 1 - max(0, p.blink_t) / max(0.01, p.blink_interval)
        flash = p.blinking > 0
        self.hud_bar(x0, y0 + 60, "МОРГАНИЕ", blink, (102, 192, 244),
                     "СЕЙЧАС" if flash else f"{max(0, p.blink_t):.1f}с", warn=p.blink_t < 1.6)

        # подсказки
        text(s, "WASD движение · Shift бег · F фонарь · Esc пауза", (18, SCREEN_H - 24),
             11, (86, 100, 112))

        # тосты
        for i, tst in enumerate(reversed(self.toasts)):
            a = min(1.0, tst["t"] * 2.0)
            img_font = font(14, bold=True)
            img = img_font.render(tst["text"], True, tst["color"])
            w = img.get_width() + 28
            rect = pygame.Rect(0, 0, w, 30)
            rect.center = (SCREEN_W // 2, SCREEN_H - 150 - i * 36)
            panel(s, rect, alpha=int(200 * a))
            text(s, tst["text"], rect.center, 14, tst["color"], bold=True, center=True,
                 alpha=int(255 * a))

        # предупреждение
        if self.statue.active and not self.statue.frozen and p.pos.distance_to(self.statue.pos) < 520:
            a = int(120 + 100 * math.sin(self.t * 9))
            text(s, "SCP-173 ВНЕ ПОЛЯ ЗРЕНИЯ", (SCREEN_W // 2, 92), 16,
                 (255, 90, 80), bold=True, center=True, alpha=max(0, a), shadow=True)

    def hud_bar(self, x, y, label, val, color, valtext, warn=False):
        s = self.screen
        val = max(0.0, min(1.0, val))
        text(s, label, (x, y - 12), 10, C_MUTED, bold=True)
        r = pygame.Rect(x, y, 220, 14)
        pygame.draw.rect(s, (12, 17, 23), r, border_radius=3)
        if val > 0:
            pygame.draw.rect(s, color, (r.x + 1, r.y + 1, int((r.w - 2) * val), r.h - 2), border_radius=2)
        pygame.draw.rect(s, (44, 58, 72), r, 1, border_radius=3)
        if warn:
            pygame.draw.rect(s, (255, 90, 80), r, 1, border_radius=3)
        text(s, valtext, (r.right + 8, y), 11, (200, 214, 226), mono=True)

    def draw_pause(self):
        s = self.screen
        o = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        o.fill((6, 9, 13, 205))
        s.blit(o, (0, 0))
        text(s, "ПАУЗА", (SCREEN_W // 2, 250), 46, (240, 246, 250), bold=True, center=True)
        lines = ["WASD — движение", "Мышь — взгляд и фонарь", "Shift — бег",
                 "F — фонарь", "Esc — продолжить"]
        for i, ln in enumerate(lines):
            text(s, ln, (SCREEN_W // 2, 340 + i * 30), 16, C_TEXT, center=True)
        text(s, "Выйти в меню — Backspace", (SCREEN_W // 2, 520), 13, C_MUTED, center=True)

    def draw_dead(self):
        s = self.screen
        a = min(1.0, self.death_t * 1.6)
        o = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        o.fill((40, 0, 0, int(170 * a)))
        s.blit(o, (0, 0))
        text(s, "ШЕЯ СЛОМАНА", (SCREEN_W // 2, 280), 52, (230, 60, 50), bold=True,
             center=True, alpha=int(255 * a), shadow=True)
        text(s, "SCP-173 не любит, когда от него отворачиваются.",
             (SCREEN_W // 2, 345), 17, (240, 200, 200), center=True, alpha=int(220 * a))
        text(s, f"Продержались: {fmt_time(self.run_time)}", (SCREEN_W // 2, 390),
             15, C_MUTED, center=True, alpha=int(200 * a))
        if self.death_t > 0.8:
            text(s, "R — попробовать снова      Esc — меню", (SCREEN_W // 2, 480),
                 15, C_TEXT, bold=True, center=True)

    def draw_win(self):
        s = self.screen
        a = min(1.0, self.win_t * 1.6)
        o = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        o.fill((0, 30, 12, int(180 * a)))
        s.blit(o, (0, 0))
        text(s, "ПОБЕГ УСПЕШЕН", (SCREEN_W // 2, 280), 48, (150, 240, 130), bold=True,
             center=True, alpha=int(255 * a), shadow=True)
        text(s, "Вы выбрались из комплекса. Фонд уже знает ваше имя.",
             (SCREEN_W // 2, 342), 17, (210, 240, 210), center=True, alpha=int(220 * a))
        text(s, f"Время: {fmt_time(self.run_time)}", (SCREEN_W // 2, 388), 15,
             C_MUTED, center=True)
        if self.win_t > 0.8:
            text(s, "R — ещё раз      Esc — меню", (SCREEN_W // 2, 480), 15,
                 C_TEXT, bold=True, center=True)

    def draw_menu(self):
        s = self.screen
        s.fill((8, 10, 14))
        # красное свечение
        glow = pygame.Surface((SCREEN_W, SCREEN_H), pygame.SRCALPHA)
        cx, cy = SCREEN_W - 260, SCREEN_H // 2 + 20
        for i in range(24, 0, -1):
            rad = int(40 + 18 * i)
            a = int(3 + (24 - i) * 1.1)
            pygame.draw.circle(glow, (110, 8, 8, a), (cx, cy), rad)
        s.blit(glow, (0, 0))
        pygame.draw.circle(s, (60, 10, 10), (cx, cy), 120, 2)
        pygame.draw.circle(s, (40, 8, 8), (cx, cy), 170, 1)
        text(s, "SCP", (cx, cy), 68, (120, 20, 20), bold=True, center=True)
        text(s, "SECURE · CONTAIN · PROTECT", (cx, cy + 62), 10, (90, 22, 22), center=True)

        # сетка
        for x in range(0, SCREEN_W, 64):
            pygame.draw.line(s, (14, 17, 22), (x, 0), (x, SCREEN_H))
        for y in range(0, SCREEN_H, 64):
            pygame.draw.line(s, (14, 17, 22), (0, y), (SCREEN_W, y))

        text(s, "SCP: SECRET LABORATORY", (70, 190), 44, (240, 245, 250), bold=True)
        text(s, "2D-прототип · побег из комплекса", (72, 246), 17, C_MUTED)
        pygame.draw.rect(s, C_RED, (72, 282, 220, 3))

        text(s, "ПРОТОКОЛ", (72, 320), 11, C_MUTED, bold=True)
        lore = [
            "Вы — сотрудник класса D. Комплекс потерял контроль.",
            "Соберите карты доступа 1–3 уровня и доберитесь до шлюза.",
            "SCP-173 замирает, пока вы на него смотрите. Моргнёте — он ударит.",
        ]
        for i, ln in enumerate(lore):
            text(s, ln, (72, 342 + i * 24), 14, (170, 184, 196))

        text(s, "УПРАВЛЕНИЕ", (72, 448), 11, C_MUTED, bold=True)
        ctr = ["WASD — движение          Shift — бег",
               "Мышь — взгляд / фонарь    F — фонарь",
               "Esc — пауза               R — заново"]
        for i, ln in enumerate(ctr):
            text(s, ln, (72, 470 + i * 24), 13, (150, 164, 178))

        if int(self.t * 2) % 2 == 0:
            text(s, "ENTER — НАЧАТЬ", (72, 590), 22, (235, 244, 250), bold=True)
        text(s, "Фанатский прототип. Не связан с Northwood Studios.",
             (72, SCREEN_H - 34), 11, (80, 92, 104))

        s.blit(self.vignette, (0, 0))

    # ---------- главный цикл ----------
    def run(self):
        running = True
        while running:
            dt = min(0.05, self.clock.tick(FPS) / 1000.0)
            for e in pygame.event.get():
                if e.type == pygame.QUIT:
                    running = False
                elif e.type == pygame.KEYDOWN:
                    if e.key == pygame.K_ESCAPE:
                        if self.state == "play":
                            self.state = "pause"
                        elif self.state == "pause":
                            self.state = "play"
                        elif self.state in ("dead", "win"):
                            self.state = "menu"
                    elif e.key == pygame.K_RETURN and self.state == "menu":
                        self.start()
                    elif e.key == pygame.K_r and self.state in ("dead", "win"):
                        self.start()
                    elif e.key == pygame.K_BACKSPACE and self.state == "pause":
                        self.state = "menu"
                    elif e.key == pygame.K_f and self.state == "play":
                        if self.player.battery > 0:
                            self.player.flash = not self.player.flash
                            self.toast("ФОНАРЬ ВКЛ" if self.player.flash else "ФОНАРЬ ВЫКЛ",
                                       C_YELLOW if self.player.flash else C_MUTED)
                        else:
                            self.toast("БАТАРЕЯ ПУСТА", C_RED)
                elif e.type == pygame.MOUSEBUTTONDOWN and self.state == "menu":
                    self.start()

            if self.state not in ("pause", "menu"):
                self.update(dt)
            else:
                self.update(dt * 0)  # только тосты/камера

            self.draw()
            pygame.display.flip()

        pygame.quit()


def make_screenshot(path, menu=False):
    g = Game()
    if menu:
        g.state = "menu"
        g.t = 1.3
        g.draw()
    else:
        g.start()
        g.player.pos = Vector2(6.6 * TS, 8.0 * TS)
        g.player.angle = -math.pi / 2 - 0.08
        g.player.flash = True
        g.statue.pos = Vector2(6.4 * TS, 4.3 * TS)
        g.statue.active = True
        g.statue.face = math.pi / 2
        g.camera = g.camera_target()
        g.mouse = (int(g.player.pos.x - g.camera.x),
                   int(g.player.pos.y - g.camera.y - 260))
        g.update(0.016)
        g.update(0.016)
        g.player.angle = -math.pi / 2 - 0.04
        g.statue.frozen = True
        g.threat = 0.0
        g.draw()
    pygame.image.save(g.screen, path)
    pygame.quit()
    print("saved", path)


def main():
    if SHOT_PATH:
        make_screenshot(SHOT_PATH, MENU_SHOT)
        return
    Game().run()


if __name__ == "__main__":
    main()
