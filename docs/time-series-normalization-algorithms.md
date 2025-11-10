# Time-Series Normalization Algorithms

**Companion Document to Architecture Specification**
**Version:** 1.0

---

## Table of Contents

1. [Core Algorithms](#core-algorithms)
2. [Timestamp Alignment Algorithms](#timestamp-alignment-algorithms)
3. [Gap Detection and Filling](#gap-detection-and-filling)
4. [Interpolation Algorithms](#interpolation-algorithms)
5. [Buffer Management Algorithms](#buffer-management-algorithms)
6. [Watermark Integration](#watermark-integration)
7. [Performance Optimizations](#performance-optimizations)

---

## Core Algorithms

### Main Normalization Pipeline

```pseudocode
ALGORITHM: NormalizeTimeSeries
INPUT:
  - event: Raw event data
  - timestamp: Event timestamp
  - key: Aggregation key
  - config: NormalizationConfig
OUTPUT:
  - normalized_events: List of normalized events

PROCEDURE:
1. // Check if event is late
   current_watermark ← watermark_tracker.get_watermark()
   IF timestamp < current_watermark THEN
       IF NOT config.allow_late_events THEN
           RETURN ERROR(LateEvent)
       END IF
       metrics.record_late_event(timestamp)
   END IF

2. // Buffer the event
   buffer.insert(key, timestamp, event)

3. // Check if we can emit normalized events
   ready_events ← buffer.get_ready_events(key, current_watermark)

   IF ready_events.is_empty() THEN
       RETURN []
   END IF

4. // Align timestamps
   aligned_events ← align_timestamps(ready_events, config.alignment_strategy)

5. // Detect and fill gaps
   filled_events ← fill_gaps(aligned_events, config.fill_strategy, config.interval)

6. // Apply interpolation if needed
   IF config.interpolation_method != NONE THEN
       interpolated_events ← interpolate(filled_events, config.interpolation_method)
   ELSE
       interpolated_events ← filled_events
   END IF

7. // Clean up buffer
   buffer.remove_before(key, current_watermark)

8. RETURN interpolated_events

COMPLEXITY:
  Time: O(n log n) where n is number of buffered events
  Space: O(n) for buffering
```

---

## Timestamp Alignment Algorithms

### Round Down Alignment

```pseudocode
ALGORITHM: RoundDownAlignment
INPUT:
  - timestamp: Event timestamp (milliseconds since epoch)
  - interval: Normalization interval (milliseconds)
OUTPUT:
  - aligned_timestamp: Aligned timestamp

PROCEDURE:
1. aligned_ms ← (timestamp / interval) * interval
2. RETURN aligned_ms

EXAMPLE:
  timestamp = 12345 ms
  interval = 5000 ms
  aligned = (12345 / 5000) * 5000 = 10000 ms

COMPLEXITY: O(1)
```

### Round Up Alignment

```pseudocode
ALGORITHM: RoundUpAlignment
INPUT:
  - timestamp: Event timestamp (milliseconds)
  - interval: Normalization interval (milliseconds)
OUTPUT:
  - aligned_timestamp: Aligned timestamp

PROCEDURE:
1. aligned_ms ← ((timestamp + interval - 1) / interval) * interval
2. RETURN aligned_ms

EXAMPLE:
  timestamp = 10001 ms
  interval = 5000 ms
  aligned = ((10001 + 4999) / 5000) * 5000 = 15000 ms

COMPLEXITY: O(1)
```

### Round Nearest Alignment

```pseudocode
ALGORITHM: RoundNearestAlignment
INPUT:
  - timestamp: Event timestamp (milliseconds)
  - interval: Normalization interval (milliseconds)
OUTPUT:
  - aligned_timestamp: Aligned timestamp

PROCEDURE:
1. half_interval ← interval / 2
2. aligned_ms ← ((timestamp + half_interval) / interval) * interval
3. RETURN aligned_ms

EXAMPLE:
  timestamp = 12000 ms
  interval = 5000 ms
  half = 2500 ms
  aligned = ((12000 + 2500) / 5000) * 5000 = 10000 ms

  timestamp = 13000 ms
  aligned = ((13000 + 2500) / 5000) * 5000 = 15000 ms

COMPLEXITY: O(1)
```

### Offset-Based Alignment

```pseudocode
ALGORITHM: OffsetBasedAlignment
INPUT:
  - timestamp: Event timestamp (milliseconds)
  - interval: Normalization interval (milliseconds)
  - offset: Offset from epoch (milliseconds)
OUTPUT:
  - aligned_timestamp: Aligned timestamp

PROCEDURE:
1. adjusted_timestamp ← timestamp - offset
2. aligned_adjusted ← (adjusted_timestamp / interval) * interval
3. aligned_ms ← aligned_adjusted + offset
4. RETURN aligned_ms

EXAMPLE:
  timestamp = 12345 ms
  interval = 5000 ms
  offset = 500 ms

  adjusted = 12345 - 500 = 11845 ms
  aligned_adjusted = (11845 / 5000) * 5000 = 10000 ms
  aligned = 10000 + 500 = 10500 ms

USE CASE: Align to custom time boundaries (e.g., start of business day)

COMPLEXITY: O(1)
```

---

## Gap Detection and Filling

### Gap Detection

```pseudocode
ALGORITHM: DetectGaps
INPUT:
  - events: Sorted list of timestamped events
  - interval: Expected interval between events
OUTPUT:
  - gaps: List of (start, end, event_before, event_after) tuples

PROCEDURE:
1. gaps ← []
2. FOR i FROM 0 TO events.length - 2 DO
       current_time ← events[i].timestamp
       next_time ← events[i+1].timestamp
       expected_next ← current_time + interval

       IF next_time > expected_next THEN
           gap ← (current_time, next_time, events[i], events[i+1])
           gaps.append(gap)
       END IF
   END FOR
3. RETURN gaps

COMPLEXITY: O(n) where n is number of events
```

### Forward Fill Strategy

```pseudocode
ALGORITHM: ForwardFill
INPUT:
  - event_before: Last known event
  - event_after: Next known event
  - interval: Normalization interval
OUTPUT:
  - filled_events: List of filled events

PROCEDURE:
1. filled_events ← []
2. current_time ← event_before.timestamp + interval
3. WHILE current_time < event_after.timestamp DO
       filled_event ← {
           timestamp: current_time,
           value: event_before.value,  // Use previous value
           key: event_before.key,
           is_filled: true
       }
       filled_events.append(filled_event)
       current_time ← current_time + interval
   END WHILE
4. RETURN filled_events

EXAMPLE:
  event_before = {timestamp: 0, value: 100}
  event_after = {timestamp: 15000, value: 200}
  interval = 5000

  Output:
    [{timestamp: 5000, value: 100},
     {timestamp: 10000, value: 100}]

COMPLEXITY: O(gap_size / interval)
```

### Backward Fill Strategy

```pseudocode
ALGORITHM: BackwardFill
INPUT:
  - event_before: Last known event
  - event_after: Next known event
  - interval: Normalization interval
OUTPUT:
  - filled_events: List of filled events

PROCEDURE:
1. filled_events ← []
2. current_time ← event_before.timestamp + interval
3. WHILE current_time < event_after.timestamp DO
       filled_event ← {
           timestamp: current_time,
           value: event_after.value,  // Use next value
           key: event_before.key,
           is_filled: true
       }
       filled_events.append(filled_event)
       current_time ← current_time + interval
   END WHILE
4. RETURN filled_events

EXAMPLE:
  event_before = {timestamp: 0, value: 100}
  event_after = {timestamp: 15000, value: 200}
  interval = 5000

  Output:
    [{timestamp: 5000, value: 200},
     {timestamp: 10000, value: 200}]

COMPLEXITY: O(gap_size / interval)
```

### Linear Interpolation Strategy

```pseudocode
ALGORITHM: LinearInterpolation
INPUT:
  - event_before: Last known event
  - event_after: Next known event
  - interval: Normalization interval
OUTPUT:
  - filled_events: List of interpolated events

PROCEDURE:
1. filled_events ← []
2. time_diff ← event_after.timestamp - event_before.timestamp
3. value_diff ← event_after.value - event_before.value
4. current_time ← event_before.timestamp + interval
5. WHILE current_time < event_after.timestamp DO
       elapsed ← current_time - event_before.timestamp
       ratio ← elapsed / time_diff
       interpolated_value ← event_before.value + (value_diff * ratio)

       filled_event ← {
           timestamp: current_time,
           value: interpolated_value,
           key: event_before.key,
           is_filled: true,
           interpolation_method: LINEAR
       }
       filled_events.append(filled_event)
       current_time ← current_time + interval
   END WHILE
6. RETURN filled_events

EXAMPLE:
  event_before = {timestamp: 0, value: 100}
  event_after = {timestamp: 10000, value: 200}
  interval = 2000

  Output:
    [{timestamp: 2000, value: 120},  // 100 + (200-100) * 0.2
     {timestamp: 4000, value: 140},  // 100 + (200-100) * 0.4
     {timestamp: 6000, value: 160},  // 100 + (200-100) * 0.6
     {timestamp: 8000, value: 180}]  // 100 + (200-100) * 0.8

COMPLEXITY: O(gap_size / interval)
```

---

## Interpolation Algorithms

### Polynomial Interpolation (Lagrange)

```pseudocode
ALGORITHM: LagrangeInterpolation
INPUT:
  - points: List of (timestamp, value) pairs
  - target_timestamp: Timestamp to interpolate
  - order: Polynomial order (number of points - 1)
OUTPUT:
  - interpolated_value: Interpolated value

PROCEDURE:
1. n ← points.length
2. result ← 0
3. FOR i FROM 0 TO n-1 DO
       term ← points[i].value
       FOR j FROM 0 TO n-1 DO
           IF i != j THEN
               term ← term * (target_timestamp - points[j].timestamp)
               term ← term / (points[i].timestamp - points[j].timestamp)
           END IF
       END FOR
       result ← result + term
   END FOR
4. RETURN result

EXAMPLE (Quadratic):
  points = [(0, 1), (1, 2), (2, 5)]
  target = 1.5

  L0 = ((1.5-1)*(1.5-2)) / ((0-1)*(0-2)) = 0.125
  L1 = ((1.5-0)*(1.5-2)) / ((1-0)*(1-2)) = 0.75
  L2 = ((1.5-0)*(1.5-1)) / ((2-0)*(2-1)) = 0.375

  result = 1*0.125 + 2*0.75 + 5*0.375 = 3.5

COMPLEXITY: O(n²) where n is number of points
```

### Spline Interpolation (Cubic)

```pseudocode
ALGORITHM: CubicSplineInterpolation
INPUT:
  - points: List of (timestamp, value) pairs (must be sorted)
  - target_timestamps: List of timestamps to interpolate
OUTPUT:
  - interpolated_values: List of interpolated values

PROCEDURE:
1. // Compute spline coefficients
   n ← points.length - 1
   h[i] ← points[i+1].timestamp - points[i].timestamp FOR i = 0 TO n-1

2. // Build tridiagonal system for second derivatives
   A ← tridiagonal_matrix(n+1)
   b ← vector(n+1)

   FOR i FROM 1 TO n-1 DO
       A[i][i-1] ← h[i-1]
       A[i][i] ← 2 * (h[i-1] + h[i])
       A[i][i+1] ← h[i]
       b[i] ← 6 * ((points[i+1].value - points[i].value) / h[i] -
                    (points[i].value - points[i-1].value) / h[i-1])
   END FOR

   // Natural spline boundary conditions
   A[0][0] ← 1
   A[n][n] ← 1
   b[0] ← 0
   b[n] ← 0

3. // Solve for second derivatives
   M ← solve_tridiagonal(A, b)

4. // Interpolate at target timestamps
   interpolated_values ← []
   FOR target IN target_timestamps DO
       // Find interval
       i ← find_interval(points, target)

       // Compute cubic polynomial
       t ← (target - points[i].timestamp) / h[i]
       a ← points[i].value
       b ← (points[i+1].value - points[i].value) / h[i] - h[i] * (2*M[i] + M[i+1]) / 6
       c ← M[i] / 2
       d ← (M[i+1] - M[i]) / (6 * h[i])

       value ← a + b*t + c*t² + d*t³
       interpolated_values.append(value)
   END FOR

5. RETURN interpolated_values

COMPLEXITY: O(n) for setup + O(log n) per interpolation
```

---

## Buffer Management Algorithms

### Circular Buffer Implementation

```pseudocode
ALGORITHM: CircularBuffer
DESCRIPTION: Memory-efficient buffer with fixed capacity

STRUCTURE:
  - buffer: Array of fixed size
  - head: Index of oldest element
  - tail: Index of next insertion
  - size: Current number of elements
  - capacity: Maximum capacity

OPERATIONS:

INSERT(element):
1. IF size == capacity THEN
       // Evict oldest element
       head ← (head + 1) MOD capacity
       size ← size - 1
   END IF
2. buffer[tail] ← element
3. tail ← (tail + 1) MOD capacity
4. size ← size + 1

GET(index):
1. IF index >= size THEN
       RETURN ERROR(IndexOutOfBounds)
   END IF
2. actual_index ← (head + index) MOD capacity
3. RETURN buffer[actual_index]

REMOVE_OLDEST(count):
1. count ← MIN(count, size)
2. head ← (head + count) MOD capacity
3. size ← size - count

COMPLEXITY:
  Insert: O(1)
  Get: O(1)
  Remove: O(1)
```

### Priority-Based Eviction

```pseudocode
ALGORITHM: PriorityEviction
INPUT:
  - buffer: Current buffer state
  - new_event: Event to insert
  - capacity: Maximum capacity
OUTPUT:
  - evicted_events: List of evicted events

PROCEDURE:
1. IF buffer.size < capacity THEN
       buffer.insert(new_event)
       RETURN []
   END IF

2. // Compute priorities for all events
   priorities ← []
   FOR event IN buffer DO
       priority ← compute_priority(event)
       priorities.append((priority, event))
   END FOR

3. // Sort by priority (ascending)
   priorities.sort()

4. // Evict lowest priority events
   evicted ← []
   WHILE buffer.size >= capacity DO
       lowest_priority_event ← priorities.pop_first()
       buffer.remove(lowest_priority_event)
       evicted.append(lowest_priority_event)
   END WHILE

5. buffer.insert(new_event)
6. RETURN evicted

FUNCTION compute_priority(event):
  // Priority factors:
  // - Recency (newer = higher priority)
  // - Proximity to watermark (closer = lower priority)
  // - Event importance (application-specific)

  recency_score ← (current_time - event.timestamp) / max_age
  watermark_distance ← (event.timestamp - watermark) / interval
  importance_score ← event.importance_weight

  priority ← recency_score * 0.4 +
              watermark_distance * 0.4 +
              importance_score * 0.2

  RETURN priority

COMPLEXITY: O(n log n) where n is buffer size
```

### Sliding Window Buffer

```pseudocode
ALGORITHM: SlidingWindowBuffer
INPUT:
  - window_size: Size of the sliding window
  - retention: Time-based retention policy
OUTPUT:
  - buffer: Managed buffer with automatic cleanup

STRUCTURE:
  - events: TreeMap<Timestamp, List<Event>>
  - oldest_timestamp: Timestamp
  - newest_timestamp: Timestamp

OPERATIONS:

INSERT(event, timestamp):
1. // Check retention
   IF current_time - timestamp > retention THEN
       RETURN ERROR(EventTooOld)
   END IF

2. // Add to appropriate bucket
   IF NOT events.contains(timestamp) THEN
       events[timestamp] ← []
   END IF
   events[timestamp].append(event)

3. // Update boundaries
   IF timestamp < oldest_timestamp OR oldest_timestamp IS NULL THEN
       oldest_timestamp ← timestamp
   END IF
   IF timestamp > newest_timestamp OR newest_timestamp IS NULL THEN
       newest_timestamp ← timestamp
   END IF

4. // Cleanup old events
   cleanup_threshold ← current_time - retention
   WHILE oldest_timestamp < cleanup_threshold DO
       events.remove(oldest_timestamp)
       oldest_timestamp ← events.first_key()
   END WHILE

GET_RANGE(start, end):
1. result ← []
2. FOR timestamp, event_list IN events.range(start, end) DO
       result.extend(event_list)
   END FOR
3. RETURN result

COMPLEXITY:
  Insert: O(log n) + cleanup
  Get Range: O(k + log n) where k is number of events in range
```

---

## Watermark Integration

### Watermark-Aware Event Processing

```pseudocode
ALGORITHM: WatermarkAwareProcessing
INPUT:
  - event: New event
  - timestamp: Event timestamp
  - watermark: Current watermark
  - config: Configuration
OUTPUT:
  - processing_decision: (PROCESS, BUFFER, or DROP)

PROCEDURE:
1. lateness ← watermark - timestamp

2. IF lateness <= 0 THEN
       // Event is on-time
       RETURN PROCESS
   END IF

3. IF lateness > config.max_lateness THEN
       // Event is too late
       metrics.record_dropped_event(lateness)

       IF config.drop_late_events THEN
           RETURN DROP
       ELSE
           // Buffer for potential reprocessing
           RETURN BUFFER
       END IF
   END IF

4. // Event is late but within allowed lateness
   metrics.record_late_event(lateness)

   IF config.allow_late_events THEN
       RETURN PROCESS
   ELSE
       RETURN BUFFER
   END IF

COMPLEXITY: O(1)
```

### Multi-Partition Watermark Merging

```pseudocode
ALGORITHM: MergePartitionWatermarks
INPUT:
  - partition_watermarks: Map<PartitionID, Watermark>
  - idle_timeout: Timeout for idle partitions
OUTPUT:
  - global_watermark: Merged global watermark

PROCEDURE:
1. active_watermarks ← []
2. current_time ← now()

3. FOR partition_id, watermark IN partition_watermarks DO
       last_activity ← partition_last_activity[partition_id]

       IF current_time - last_activity <= idle_timeout THEN
           // Partition is active
           active_watermarks.append(watermark)
       ELSE
           // Partition is idle, don't include in merge
           metrics.record_idle_partition(partition_id)
       END IF
   END FOR

4. IF active_watermarks.is_empty() THEN
       // All partitions idle, don't advance watermark
       RETURN current_global_watermark
   END IF

5. // Global watermark is minimum of active partitions
   global_watermark ← MIN(active_watermarks)

6. IF global_watermark > current_global_watermark THEN
       RETURN global_watermark
   ELSE
       RETURN current_global_watermark
   END IF

COMPLEXITY: O(p) where p is number of partitions
```

---

## Performance Optimizations

### Zero-Copy Timestamp Alignment

```pseudocode
ALGORITHM: ZeroCopyAlignment
DESCRIPTION: Avoid memory allocation during alignment

PROCEDURE:
1. // Pre-compute interval in milliseconds
   interval_ms ← interval.to_milliseconds()

2. // Batch alignment using SIMD (vectorized operations)
   batch_size ← 8  // SIMD vector width

3. FOR i FROM 0 TO timestamps.length STEP batch_size DO
       // Load timestamps into vector register
       vec ← SIMD_LOAD(timestamps[i:i+batch_size])

       // Vectorized division and multiplication
       aligned_vec ← (vec / interval_ms) * interval_ms

       // Store back
       SIMD_STORE(aligned_timestamps[i:i+batch_size], aligned_vec)
   END FOR

4. // Handle remaining timestamps
   FOR i FROM (timestamps.length / batch_size) * batch_size TO timestamps.length DO
       aligned_timestamps[i] ← (timestamps[i] / interval_ms) * interval_ms
   END FOR

OPTIMIZATION: 8x speedup with AVX2, 16x with AVX-512
```

### Batch Gap Filling

```pseudocode
ALGORITHM: BatchGapFilling
INPUT:
  - events: Sorted list of events
  - interval: Normalization interval
  - strategy: Fill strategy
OUTPUT:
  - filled_events: Events with gaps filled

PROCEDURE:
1. // Detect all gaps in one pass
   gaps ← detect_all_gaps(events, interval)

2. IF gaps.is_empty() THEN
       RETURN events
   END IF

3. // Pre-allocate result buffer
   estimated_size ← events.length + sum(gap.size for gap in gaps)
   result ← allocate_vec(estimated_size)

4. // Fill gaps in batch
   event_idx ← 0
   FOR gap IN gaps DO
       // Copy events before gap
       WHILE events[event_idx].timestamp < gap.start DO
           result.append(events[event_idx])
           event_idx ← event_idx + 1
       END WHILE

       // Fill gap
       filled ← strategy.fill_gap(gap)
       result.extend(filled)
   END FOR

5. // Copy remaining events
   WHILE event_idx < events.length DO
       result.append(events[event_idx])
       event_idx ← event_idx + 1
   END WHILE

6. RETURN result

OPTIMIZATION: Single allocation, single pass, cache-friendly
COMPLEXITY: O(n + g*f) where n=events, g=gaps, f=fill_size
```

### Parallel Event Processing

```pseudocode
ALGORITHM: ParallelEventProcessing
INPUT:
  - events: Batch of events to process
  - num_threads: Degree of parallelism
  - normalizer: Normalization configuration
OUTPUT:
  - normalized_events: Processed events

PROCEDURE:
1. // Partition events by key for parallelism
   key_partitions ← partition_by_key(events, num_threads)

2. // Process partitions in parallel
   futures ← []
   FOR partition IN key_partitions DO
       future ← spawn_async({
           normalized ← []
           FOR event IN partition DO
               result ← normalizer.process_event(event)
               normalized.extend(result)
           END FOR
           RETURN normalized
       })
       futures.append(future)
   END FOR

3. // Collect results
   all_normalized ← []
   FOR future IN futures DO
       result ← await future
       all_normalized.extend(result)
   END FOR

4. // Merge and sort
   all_normalized.sort_by_timestamp()

5. RETURN all_normalized

OPTIMIZATION: N-way parallelism with key-based partitioning
COMPLEXITY: O(n/p * log(n/p)) where p is parallelism
```

---

## Edge Case Handling

### Duplicate Timestamp Handling

```pseudocode
ALGORITHM: HandleDuplicateTimestamps
INPUT:
  - events: Events with potentially duplicate timestamps
  - aggregation_method: How to combine duplicates
OUTPUT:
  - deduplicated_events: Events with unique timestamps

PROCEDURE:
1. grouped ← group_by_timestamp(events)

2. result ← []
3. FOR timestamp, event_group IN grouped DO
       IF event_group.length == 1 THEN
           result.append(event_group[0])
       ELSE
           // Aggregate duplicates based on method
           CASE aggregation_method:
               AVERAGE:
                   combined ← average_values(event_group)
               SUM:
                   combined ← sum_values(event_group)
               FIRST:
                   combined ← event_group[0]
               LAST:
                   combined ← event_group[-1]
               MIN:
                   combined ← min_value(event_group)
               MAX:
                   combined ← max_value(event_group)
           END CASE
           result.append(combined)
       END IF
   END FOR

4. RETURN result

COMPLEXITY: O(n log n) for grouping + O(n) for aggregation
```

### Boundary Condition Handling

```pseudocode
ALGORITHM: HandleBoundaryConditions
INPUT:
  - event: Event at window boundary
  - window_start: Window start timestamp
  - window_end: Window end timestamp
  - boundary_policy: How to handle boundaries
OUTPUT:
  - windows: List of windows event belongs to

PROCEDURE:
1. timestamp ← event.timestamp

2. CASE boundary_policy:
       LEFT_CLOSED_RIGHT_OPEN:
           IF timestamp >= window_start AND timestamp < window_end THEN
               RETURN [window]
           END IF

       LEFT_OPEN_RIGHT_CLOSED:
           IF timestamp > window_start AND timestamp <= window_end THEN
               RETURN [window]
           END IF

       BOTH_CLOSED:
           IF timestamp >= window_start AND timestamp <= window_end THEN
               RETURN [window]
           END IF

       BOTH_OPEN:
           IF timestamp > window_start AND timestamp < window_end THEN
               RETURN [window]
           END IF

       DUPLICATE_AT_BOUNDARY:
           // Event belongs to both adjacent windows
           IF timestamp == window_end THEN
               next_window ← create_window(window_end, window_end + interval)
               RETURN [window, next_window]
           END IF
   END CASE

3. RETURN []

COMPLEXITY: O(1)
```

---

## Conclusion

These algorithms provide the mathematical and computational foundation for the Time-Series Normalization system. Key characteristics:

- **Efficiency**: O(1) or O(log n) for most operations
- **Scalability**: Parallel processing support
- **Correctness**: Handles edge cases and boundary conditions
- **Performance**: Zero-copy and SIMD optimizations

The pseudocode can be directly translated to Rust implementations with minimal adaptation.
