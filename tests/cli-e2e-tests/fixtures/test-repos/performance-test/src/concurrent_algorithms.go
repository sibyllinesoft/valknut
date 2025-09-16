package main

import (
	"fmt"
	"math/rand"
	"runtime"
	"sync"
	"time"
)

// ComplexStruct represents a complex data structure for testing
type ComplexStruct struct {
	ID       int
	Name     string
	Values   []float64
	Children map[string]*ComplexStruct
	mutex    sync.RWMutex
}

// NewComplexStruct creates a new complex struct with random data
func NewComplexStruct(id int, depth int) *ComplexStruct {
	cs := &ComplexStruct{
		ID:       id,
		Name:     fmt.Sprintf("struct_%d", id),
		Values:   make([]float64, rand.Intn(100)+10),
		Children: make(map[string]*ComplexStruct),
	}

	// Fill values with random data
	for i := range cs.Values {
		cs.Values[i] = rand.Float64() * 1000
	}

	// Create children recursively (limited depth to prevent stack overflow)
	if depth > 0 {
		numChildren := rand.Intn(5) + 1
		for i := 0; i < numChildren; i++ {
			childID := id*10 + i
			childKey := fmt.Sprintf("child_%d", i)
			cs.Children[childKey] = NewComplexStruct(childID, depth-1)
		}
	}

	return cs
}

// ProcessValues performs complex calculations on values
func (cs *ComplexStruct) ProcessValues() float64 {
	cs.mutex.RLock()
	defer cs.mutex.RUnlock()

	var result float64
	for i, value := range cs.Values {
		// Complex mathematical operations
		if i%2 == 0 {
			result += value * value
		} else {
			result += value / (float64(i) + 1)
		}

		// Simulate some processing time
		for j := 0; j < 1000; j++ {
			result += float64(j) * 0.001
		}
	}

	return result
}

// ProcessChildrenConcurrently processes all children concurrently
func (cs *ComplexStruct) ProcessChildrenConcurrently() map[string]float64 {
	cs.mutex.RLock()
	children := make(map[string]*ComplexStruct)
	for k, v := range cs.Children {
		children[k] = v
	}
	cs.mutex.RUnlock()

	results := make(map[string]float64)
	var wg sync.WaitGroup
	var mutex sync.Mutex

	for key, child := range children {
		wg.Add(1)
		go func(k string, c *ComplexStruct) {
			defer wg.Done()
			result := c.ProcessValues()
			
			mutex.Lock()
			results[k] = result
			mutex.Unlock()
		}(key, child)
	}

	wg.Wait()
	return results
}

// DeepTraversal performs a deep traversal of the structure
func (cs *ComplexStruct) DeepTraversal(visitor func(*ComplexStruct)) {
	cs.mutex.RLock()
	defer cs.mutex.RUnlock()

	visitor(cs)

	for _, child := range cs.Children {
		child.DeepTraversal(visitor)
	}
}

// ConcurrentAlgorithms demonstrates various concurrent patterns
type ConcurrentAlgorithms struct {
	data     []*ComplexStruct
	results  map[int]float64
	mutex    sync.RWMutex
	workerWG sync.WaitGroup
}

// NewConcurrentAlgorithms creates a new instance
func NewConcurrentAlgorithms(size int) *ConcurrentAlgorithms {
	data := make([]*ComplexStruct, size)
	for i := 0; i < size; i++ {
		data[i] = NewComplexStruct(i, 3) // depth of 3
	}

	return &ConcurrentAlgorithms{
		data:    data,
		results: make(map[int]float64),
	}
}

// ParallelProcessing processes data using multiple goroutines
func (ca *ConcurrentAlgorithms) ParallelProcessing(numWorkers int) {
	jobs := make(chan *ComplexStruct, len(ca.data))
	
	// Start workers
	for w := 0; w < numWorkers; w++ {
		ca.workerWG.Add(1)
		go ca.worker(jobs)
	}

	// Send jobs
	for _, item := range ca.data {
		jobs <- item
	}
	close(jobs)

	ca.workerWG.Wait()
}

// worker processes items from the jobs channel
func (ca *ConcurrentAlgorithms) worker(jobs <-chan *ComplexStruct) {
	defer ca.workerWG.Done()
	
	for item := range jobs {
		// Simulate complex processing
		result := item.ProcessValues()
		
		// Process children concurrently
		childResults := item.ProcessChildrenConcurrently()
		for _, childResult := range childResults {
			result += childResult * 0.1
		}

		// Store result safely
		ca.mutex.Lock()
		ca.results[item.ID] = result
		ca.mutex.Unlock()
		
		// Simulate some additional processing time
		time.Sleep(time.Millisecond * time.Duration(rand.Intn(10)+1))
	}
}

// PipelineProcessing demonstrates pipeline pattern
func (ca *ConcurrentAlgorithms) PipelineProcessing() <-chan float64 {
	// Stage 1: Generate data
	stage1 := make(chan *ComplexStruct)
	go func() {
		defer close(stage1)
		for _, item := range ca.data {
			stage1 <- item
		}
	}()

	// Stage 2: Process values
	stage2 := make(chan float64)
	go func() {
		defer close(stage2)
		for item := range stage1 {
			result := item.ProcessValues()
			stage2 <- result
		}
	}()

	// Stage 3: Apply transformations
	stage3 := make(chan float64)
	go func() {
		defer close(stage3)
		for value := range stage2 {
			// Apply complex transformation
			transformed := value * 1.5
			if transformed > 10000 {
				transformed = transformed / 2
			}
			stage3 <- transformed
		}
	}()

	return stage3
}

// MemoryIntensiveOperation creates and processes large amounts of data
func (ca *ConcurrentAlgorithms) MemoryIntensiveOperation() {
	// Create large slices
	largeSlice := make([][]float64, 1000)
	for i := range largeSlice {
		largeSlice[i] = make([]float64, 1000)
		for j := range largeSlice[i] {
			largeSlice[i][j] = rand.Float64() * 100
		}
	}

	// Process in parallel
	var wg sync.WaitGroup
	numGoroutines := runtime.NumCPU()
	chunkSize := len(largeSlice) / numGoroutines

	for i := 0; i < numGoroutines; i++ {
		start := i * chunkSize
		end := start + chunkSize
		if i == numGoroutines-1 {
			end = len(largeSlice)
		}

		wg.Add(1)
		go func(start, end int) {
			defer wg.Done()
			for i := start; i < end; i++ {
				for j := range largeSlice[i] {
					// Complex calculation
					largeSlice[i][j] = largeSlice[i][j]*largeSlice[i][j] + float64(i*j)
				}
			}
		}(start, end)
	}

	wg.Wait()
}

func main() {
	fmt.Println("Starting concurrent algorithms performance test...")
	
	// Set up random seed
	rand.Seed(time.Now().UnixNano())
	
	start := time.Now()
	
	// Create algorithm suite
	ca := NewConcurrentAlgorithms(100)
	
	// Test parallel processing
	fmt.Println("Running parallel processing...")
	ca.ParallelProcessing(runtime.NumCPU())
	
	// Test pipeline processing
	fmt.Println("Running pipeline processing...")
	results := ca.PipelineProcessing()
	count := 0
	total := 0.0
	for result := range results {
		total += result
		count++
	}
	
	fmt.Printf("Pipeline processed %d items, average: %.2f\n", count, total/float64(count))
	
	// Test memory intensive operation
	fmt.Println("Running memory intensive operation...")
	ca.MemoryIntensiveOperation()
	
	elapsed := time.Since(start)
	fmt.Printf("Total execution time: %v\n", elapsed)
	fmt.Printf("Final results count: %d\n", len(ca.results))
	
	// Print memory stats
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	fmt.Printf("Memory allocated: %d KB\n", m.Alloc/1024)
	fmt.Printf("Total allocations: %d\n", m.TotalAlloc/1024)
	fmt.Printf("Number of GC runs: %d\n", m.NumGC)
}
