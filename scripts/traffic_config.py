# Traffic Generator Configuration
# This file contains all the configuration settings for the traffic generator
# Keeping them separate prevents accidental overwrites during code changes

# Global Configuration
TEMP_DIR = "/tmp/obsctl-traffic"
OBSCTL_BINARY = "../target/release/obsctl"
MINIO_ENDPOINT = "http://127.0.0.1:9000"
SCRIPT_DURATION_HOURS = 12
MAX_CONCURRENT_USERS = 10

# Traffic Volume Settings (operations per minute)
PEAK_VOLUME_MIN = 100    # Minimum ops/min during peak hours
PEAK_VOLUME_MAX = 500    # Maximum ops/min during peak hours
OFF_PEAK_VOLUME_MIN = 10 # Minimum ops/min during off-peak hours
OFF_PEAK_VOLUME_MAX = 50 # Maximum ops/min during off-peak hours

# File TTL Settings (in seconds)
REGULAR_FILE_TTL = 3 * 3600     # 3 hours for regular files
LARGE_FILE_TTL = 60 * 60        # 60 minutes for large files (>100MB)
LARGE_FILE_THRESHOLD = 100 * 1024 * 1024  # 100MB threshold

# User Configurations
USER_CONFIGS = {
    'alice-dev': {
        'description': 'Software Developer - Heavy code and docs',
        'bucket': 'alice-dev-workspace',
        'timezone_offset': 0,  # UTC
        'peak_hours': (9, 17),  # 9 AM to 5 PM
        'activity_multiplier': 1.5,
        'file_preferences': {
            'code': 0.4,
            'documents': 0.3,
            'images': 0.1,
            'archives': 0.1,
            'media': 0.1
        }
    },
    'bob-marketing': {
        'description': 'Marketing Manager - Media and presentations',
        'bucket': 'bob-marketing-assets',
        'timezone_offset': -5,  # EST
        'peak_hours': (8, 16),
        'activity_multiplier': 1.2,
        'file_preferences': {
            'media': 0.4,
            'images': 0.3,
            'documents': 0.2,
            'code': 0.05,
            'archives': 0.05
        }
    },
    'carol-data': {
        'description': 'Data Scientist - Large datasets and analysis',
        'bucket': 'carol-analytics',
        'timezone_offset': -8,  # PST
        'peak_hours': (10, 18),
        'activity_multiplier': 2.0,
        'file_preferences': {
            'archives': 0.4,
            'documents': 0.3,
            'code': 0.2,
            'images': 0.05,
            'media': 0.05
        }
    },
    'david-backup': {
        'description': 'IT Admin - Automated backup systems',
        'bucket': 'david-backups',
        'timezone_offset': 0,  # UTC
        'peak_hours': (2, 6),  # Night backup window
        'activity_multiplier': 3.0,
        'file_preferences': {
            'archives': 0.6,
            'documents': 0.2,
            'code': 0.1,
            'images': 0.05,
            'media': 0.05
        }
    },
    'eve-design': {
        'description': 'Creative Designer - Images and media files',
        'bucket': 'eve-creative-work',
        'timezone_offset': 1,  # CET
        'peak_hours': (9, 17),
        'activity_multiplier': 1.8,
        'file_preferences': {
            'images': 0.5,
            'media': 0.3,
            'documents': 0.1,
            'code': 0.05,
            'archives': 0.05
        }
    },
    'frank-research': {
        'description': 'Research Scientist - Academic papers and data',
        'bucket': 'frank-research-data',
        'timezone_offset': -3,  # BRT
        'peak_hours': (14, 22),  # Afternoon/evening researcher
        'activity_multiplier': 1.3,
        'file_preferences': {
            'documents': 0.4,
            'archives': 0.3,
            'code': 0.2,
            'images': 0.05,
            'media': 0.05
        }
    },
    'grace-sales': {
        'description': 'Sales Manager - Presentations and materials',
        'bucket': 'grace-sales-materials',
        'timezone_offset': -6,  # CST
        'peak_hours': (8, 16),
        'activity_multiplier': 1.1,
        'file_preferences': {
            'documents': 0.4,
            'images': 0.3,
            'media': 0.2,
            'code': 0.05,
            'archives': 0.05
        }
    },
    'henry-ops': {
        'description': 'DevOps Engineer - Infrastructure and configs',
        'bucket': 'henry-operations',
        'timezone_offset': 0,  # UTC
        'peak_hours': (0, 8),  # Night shift operations
        'activity_multiplier': 2.5,
        'file_preferences': {
            'code': 0.4,
            'archives': 0.3,
            'documents': 0.2,
            'images': 0.05,
            'media': 0.05
        }
    },
    'iris-content': {
        'description': 'Content Manager - Digital asset library',
        'bucket': 'iris-content-library',
        'timezone_offset': 9,  # JST
        'peak_hours': (9, 17),
        'activity_multiplier': 1.7,
        'file_preferences': {
            'media': 0.4,
            'images': 0.3,
            'documents': 0.2,
            'archives': 0.05,
            'code': 0.05
        }
    },
    'jack-mobile': {
        'description': 'Mobile Developer - App assets and code',
        'bucket': 'jack-mobile-apps',
        'timezone_offset': 5.5,  # IST
        'peak_hours': (10, 18),
        'activity_multiplier': 1.6,
        'file_preferences': {
            'code': 0.4,
            'images': 0.3,
            'media': 0.2,
            'documents': 0.05,
            'archives': 0.05
        }
    }
}

# File type extensions
FILE_EXTENSIONS = {
    'code': ['.py', '.js', '.html', '.css', '.rs', '.go', '.java', '.cpp', '.c', '.json', '.xml', '.yaml', '.toml'],
    'documents': ['.pdf', '.docx', '.txt', '.md', '.xlsx', '.pptx', '.csv', '.rtf'],
    'images': ['.jpg', '.png', '.gif', '.svg', '.bmp', '.webp', '.tiff'],
    'archives': ['.zip', '.tar.gz', '.rar', '.7z', '.tar', '.gz'],
    'media': ['.mp4', '.avi', '.mov', '.mkv', '.mp3', '.wav', '.flac', '.ogg']
}

# Environment variables for obsctl
OBSCTL_ENV = {
    'OTEL_ENABLED': 'true',
    'OTEL_EXPORTER_OTLP_ENDPOINT': 'http://localhost:4317',
    'AWS_ACCESS_KEY_ID': 'minioadmin',
    'AWS_SECRET_ACCESS_KEY': 'minioadmin123',
    'AWS_ENDPOINT_URL': MINIO_ENDPOINT,
    'AWS_REGION': 'us-east-1'
}
