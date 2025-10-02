#!/bin/bash

set -e

show_usage() {
    cat << USAGE
Network Latency Simulator for RPC Benchmarks

Usage: $0 [COMMAND] [OPTIONS]

Commands:
    add <delay>     Add network latency (e.g., 50ms, 100ms, 200ms)
    remove          Remove network latency simulation
    status          Show current network settings
    test            Run a quick latency test

Examples:
    # Simulate 50ms latency (typical same-continent)
    $0 add 50ms

    # Simulate 150ms latency (typical cross-continent)
    $0 add 150ms

    # Remove simulation
    $0 remove

Common latency scenarios (Worldwide):
    - Same datacenter:     < 1ms
    - Same city:           5-10ms
    - Same continent:      30-80ms
    - Cross-continent:     100-200ms
    - Intercontinental:    200-400ms

European User Scenarios (Server in Germany):
    - Germany (Fiber):     5-15ms   (Deutsche Telekom, Vodafone fiber)
    - Germany (DSL):       10-25ms  (VDSL connections)
    - France (Paris):      15-30ms  (Orange, Free fiber)
    - UK (London):         20-35ms  (BT, Virgin Media)
    - Netherlands:         10-20ms  (KPN, Ziggo)
    - Poland (Warsaw):     20-40ms  (Orange, Play)
    - Spain (Madrid):      30-50ms  (Movistar, Vodafone)
    - Italy (Milan):       25-45ms  (TIM, Fastweb)
    - Sweden (Stockholm):  25-40ms  (Telia, Tele2)
    - Switzerland:         15-30ms  (Swisscom, UPC)
    - Austria (Vienna):    10-20ms  (A1, Magenta)
    - Mobile 4G/5G:        30-60ms  (Additional jitter)
    - Mobile 3G:           80-150ms (High latency)
    - Rural/Satellite:     400-700ms (Very high latency)

Preset profiles:
    $0 preset eu-fiber      # 20ms - European fiber (avg)
    $0 preset eu-dsl        # 35ms - European DSL (avg)
    $0 preset eu-mobile4g   # 45ms - European 4G mobile
    $0 preset eu-mobile3g   # 100ms - European 3G mobile
    $0 preset eu-far        # 50ms - Far European countries
    $0 preset eu-rural      # 80ms - Rural European connection

Note: Requires sudo privileges on Linux. On macOS, use Network Link Conditioner.
USAGE
}

case "$1" in
    preset)
        case "$2" in
            eu-fiber)
                echo "📡 European Fiber Profile: 20ms latency + 2ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 20ms 2ms
                    echo "✅ Simulating: German/Swiss/Austrian fiber connection"
                fi
                ;;
            eu-dsl)
                echo "📡 European DSL Profile: 35ms latency + 5ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 35ms 5ms
                    echo "✅ Simulating: Average European DSL (UK, France, Germany)"
                fi
                ;;
            eu-mobile4g)
                echo "📱 European 4G Mobile Profile: 45ms latency + 10ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 45ms 10ms
                    echo "✅ Simulating: 4G/LTE mobile connection across Europe"
                fi
                ;;
            eu-mobile3g)
                echo "📱 European 3G Mobile Profile: 100ms latency + 20ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 100ms 20ms
                    echo "✅ Simulating: 3G mobile connection"
                fi
                ;;
            eu-far)
                echo "🌍 Far European Profile: 50ms latency + 8ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 50ms 8ms
                    echo "✅ Simulating: Spain/Italy/Poland to Germany"
                fi
                ;;
            eu-rural)
                echo "🏞️  Rural European Profile: 80ms latency + 15ms jitter"
                if [[ "$OSTYPE" == "linux-gnu"* ]]; then
                    sudo tc qdisc add dev lo root netem delay 80ms 15ms
                    echo "✅ Simulating: Rural DSL or satellite backup"
                fi
                ;;
            *)
                echo "Unknown preset: $2"
                show_usage
                exit 1
                ;;
        esac
        ;;
    add)
        if [ -z "$2" ]; then
            echo "Error: Please specify delay (e.g., 50ms)"
            show_usage
            exit 1
        fi

        if [[ "$OSTYPE" == "linux-gnu"* ]]; then
            echo "Adding ${2} latency to loopback interface..."
            sudo tc qdisc add dev lo root netem delay "$2"
            echo "✅ Network latency simulation active: $2"
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            echo "⚠️  macOS detected. Please use Network Link Conditioner:"
            echo "   1. Install: Xcode → Open Developer Tool → More Developer Tools"
            echo "   2. Open: /System/Library/PreferencePanes/Network Link Conditioner.prefPane"
            echo "   3. Configure custom profile with ${2} latency"
        fi
        ;;

    remove)
        if [[ "$OSTYPE" == "linux-gnu"* ]]; then
            echo "Removing network latency simulation..."
            sudo tc qdisc del dev lo root 2>/dev/null || echo "No latency simulation active"
            echo "✅ Network latency simulation removed"
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            echo "Please disable Network Link Conditioner in System Preferences"
        fi
        ;;

    status)
        if [[ "$OSTYPE" == "linux-gnu"* ]]; then
            echo "Current network configuration:"
            tc qdisc show dev lo
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            echo "Check Network Link Conditioner in System Preferences"
        fi
        ;;

    test)
        echo "Running latency test..."
        ping -c 5 127.0.0.1
        ;;

    *)
        show_usage
        ;;
esac
