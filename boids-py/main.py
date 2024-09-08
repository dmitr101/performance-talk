import pygame
import random
import math
import sys

# Initialize Pygame
pygame.init()

# Screen dimensions
WIDTH = 1070
HEIGHT = 800

# Colors
WHITE = (255, 255, 255)
RED = (255, 0, 0)
BLUE = (0, 0, 255)
BLACK = (0, 0, 0)

# Boid properties
BOID_SIZE = 10
# Accept initial boid count from command line
INITIAL_BOIDS = int(sys.argv[1] if len(sys.argv) > 1 else 100)
MAX_SPEED = 240  # pixels per second
MAX_FORCE = 200  # pixels per second
PERCEPTION = 50
SEPARATION = 50
MOUSE_PERCEPTION = 100
MOUSE_FORCE = 30  # pixels per second


class Boid:
    def __init__(self):
        self.position = pygame.Vector2(
            random.randint(0, WIDTH), random.randint(0, HEIGHT)
        )
        self.velocity = pygame.Vector2(random.uniform(-1, 1), random.uniform(-1, 1))
        self.velocity = self.velocity.normalize() * random.uniform(1, MAX_SPEED)
        self.acceleration = pygame.Vector2(0, 0)
        self.life_history = [0 for _ in range(512)]

    def update(self, dt):
        self.velocity += self.acceleration * dt
        if self.velocity.length() > MAX_SPEED:
            self.velocity = self.velocity.normalize() * MAX_SPEED
        self.position += self.velocity * dt
        self.acceleration *= 0

    def align(self, boids):
        steering = pygame.Vector2(0, 0)
        total = 0
        for boid in boids:
            distance = self.position.distance_to(boid.position)
            if boid != self and distance < PERCEPTION and distance > 0:
                steering += boid.velocity
                total += 1
        if total > 0:
            steering = steering / total
            steering = steering.normalize() * MAX_SPEED
            steering -= self.velocity
            if steering.length() > MAX_FORCE:
                steering = steering.normalize() * MAX_FORCE
        return steering

    def cohesion(self, boids):
        steering = pygame.Vector2(0, 0)
        total = 0
        for boid in boids:
            distance = self.position.distance_to(boid.position)
            if boid != self and distance < PERCEPTION and distance > 0:
                steering += boid.position
                total += 1
        if total > 0:
            steering = steering / total
            steering -= self.position
            steering = steering.normalize() * MAX_SPEED
            steering -= self.velocity
            if steering.length() > MAX_FORCE:
                steering = steering.normalize() * MAX_FORCE
        return steering

    def separation(self, boids):
        steering = pygame.Vector2(0, 0)
        total = 0
        for boid in boids:
            distance = self.position.distance_to(boid.position)
            if boid != self and distance < SEPARATION and distance > 0:
                diff = self.position - boid.position
                diff = diff.normalize() / distance
                steering += diff
                total += 1
        if total > 0:
            steering = steering / total
            steering = steering.normalize() * MAX_SPEED
            steering -= self.velocity
            if steering.length() > MAX_FORCE:
                steering = steering.normalize() * MAX_FORCE
        return steering

    def react_to_mouse(self, mouse_pos, is_attracted):
        steering = pygame.Vector2(0, 0)
        distance = self.position.distance_to(mouse_pos)

        if distance < MOUSE_PERCEPTION and distance > 0:
            diff = mouse_pos - self.position
            diff = diff.normalize()

            if is_attracted:
                steering = diff * MOUSE_FORCE
            else:
                steering = -diff * MOUSE_FORCE

            if steering.length() > MAX_FORCE:
                steering = steering.normalize() * MAX_FORCE

        return steering

    def apply_behavior(self, boids, mouse_pos, is_attracted):
        alignment = self.align(boids)
        cohesion = self.cohesion(boids)
        separation = self.separation(boids)
        mouse_reaction = self.react_to_mouse(mouse_pos, is_attracted)

        self.acceleration += alignment
        self.acceleration += cohesion
        self.acceleration += separation
        self.acceleration += mouse_reaction

    def edges(self):
        if self.position.x > WIDTH:
            self.position.x = 0
        elif self.position.x < 0:
            self.position.x = WIDTH
        if self.position.y > HEIGHT:
            self.position.y = 0
        elif self.position.y < 0:
            self.position.y = HEIGHT

    def draw(self, screen, is_attracted):
        angle = math.atan2(self.velocity.y, self.velocity.x)

        p1 = (
            self.position.x + BOID_SIZE * math.cos(angle),
            self.position.y + BOID_SIZE * math.sin(angle),
        )
        p2 = (
            self.position.x + BOID_SIZE * math.cos(angle + 2.5),
            self.position.y + BOID_SIZE * math.sin(angle + 2.5),
        )
        p3 = (
            self.position.x + BOID_SIZE * math.cos(angle - 2.5),
            self.position.y + BOID_SIZE * math.sin(angle - 2.5),
        )

        color = BLUE if is_attracted else RED
        pygame.draw.polygon(screen, color, [p1, p2, p3])


# Create boids
boids = [Boid() for _ in range(INITIAL_BOIDS)]

# Set up the display
screen = pygame.display.set_mode((WIDTH, HEIGHT))
pygame.display.set_caption("Boids Simulation")
clock = pygame.time.Clock()

# Set up font for FPS and boid count
font = pygame.font.Font(None, 36)

# Initial attraction state
is_attracted = False


def update_all_boids(
    boids: list[Boid], dt: float, mouse_pos: pygame.Vector2, is_attracted: bool
):
    for boid in boids:
        boid.apply_behavior(boids, mouse_pos, is_attracted)
        boid.update(dt)
        boid.edges()


def draw_all_boids(boids: list[Boid], screen: pygame.Surface, is_attracted: bool):
    for boid in boids:
        boid.draw(screen, is_attracted)


# Main game loop
running = True
while running:
    dt = clock.tick() / 1000.0  # Get the time elapsed since last frame in seconds

    for event in pygame.event.get():
        if event.type == pygame.QUIT:
            running = False
        elif event.type == pygame.KEYDOWN:
            if event.key == pygame.K_SPACE:
                is_attracted = not is_attracted
            elif event.key == pygame.K_UP:
                boids.extend([Boid() for _ in range(10)])
            elif event.key == pygame.K_DOWN:
                for _ in range(min(10, len(boids))):
                    boids.pop()

    screen.fill(WHITE)

    mouse_pos = pygame.Vector2(pygame.mouse.get_pos())

    update_all_boids(boids, dt, mouse_pos, is_attracted)
    draw_all_boids(boids, screen, is_attracted)

    # Calculate and draw FPS
    fps = clock.get_fps()
    fps_text = font.render(f"FPS: {fps:.2f}", True, BLACK)
    screen.blit(fps_text, (10, 10))

    # Draw boid count
    boid_count_text = font.render(f"Boids: {len(boids)}", True, BLACK)
    screen.blit(boid_count_text, (10, 50))

    pygame.display.flip()

pygame.quit()
