/**
 * ItemList component for displaying a list of items
 */
import React, { useState, useEffect, useCallback } from 'react';
import { fetchItems, deleteItem } from '../utils/api.js';
import ItemCard from './ItemCard.js';
import LoadingSpinner from './LoadingSpinner.js';
import ErrorMessage from './ErrorMessage.js';

const ItemList = ({ category = null, onItemSelect = null }) => {
    const [items, setItems] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [filter, setFilter] = useState('');
    const [sortBy, setSortBy] = useState('name');
    const [sortOrder, setSortOrder] = useState('asc');

    const loadItems = useCallback(async () => {
        try {
            setLoading(true);
            setError(null);
            
            const params = {};
            if (category) {
                params.category = category;
            }
            
            const data = await fetchItems(params);
            setItems(data);
        } catch (err) {
            console.error('Failed to load items:', err);
            setError('Failed to load items. Please try again.');
        } finally {
            setLoading(false);
        }
    }, [category]);

    useEffect(() => {
        loadItems();
    }, [loadItems]);

    const handleDeleteItem = async (itemId) => {
        if (!window.confirm('Are you sure you want to delete this item?')) {
            return;
        }

        try {
            await deleteItem(itemId);
            setItems(prevItems => prevItems.filter(item => item.id !== itemId));
        } catch (err) {
            console.error('Failed to delete item:', err);
            setError('Failed to delete item. Please try again.');
        }
    };

    const filteredAndSortedItems = items
        .filter(item => {
            if (!filter) return true;
            return (
                item.name.toLowerCase().includes(filter.toLowerCase()) ||
                item.description?.toLowerCase().includes(filter.toLowerCase()) ||
                item.category.toLowerCase().includes(filter.toLowerCase())
            );
        })
        .sort((a, b) => {
            let aValue = a[sortBy];
            let bValue = b[sortBy];

            // Handle different data types
            if (typeof aValue === 'string') {
                aValue = aValue.toLowerCase();
                bValue = bValue.toLowerCase();
            }

            if (sortOrder === 'asc') {
                return aValue < bValue ? -1 : aValue > bValue ? 1 : 0;
            } else {
                return aValue > bValue ? -1 : aValue < bValue ? 1 : 0;
            }
        });

    if (loading) {
        return <LoadingSpinner message="Loading items..." />;
    }

    if (error) {
        return (
            <ErrorMessage 
                message={error} 
                onRetry={loadItems}
            />
        );
    }

    return (
        <div className="item-list">
            <div className="item-list-header">
                <h2>
                    {category ? `${category} Items` : 'All Items'} 
                    <span className="item-count">({filteredAndSortedItems.length})</span>
                </h2>
                
                <div className="item-list-controls">
                    <input
                        type="text"
                        placeholder="Filter items..."
                        value={filter}
                        onChange={(e) => setFilter(e.target.value)}
                        className="filter-input"
                    />
                    
                    <select
                        value={sortBy}
                        onChange={(e) => setSortBy(e.target.value)}
                        className="sort-select"
                    >
                        <option value="name">Sort by Name</option>
                        <option value="price">Sort by Price</option>
                        <option value="category">Sort by Category</option>
                    </select>
                    
                    <button
                        onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
                        className="sort-order-btn"
                    >
                        {sortOrder === 'asc' ? '↑' : '↓'}
                    </button>
                    
                    <button
                        onClick={loadItems}
                        className="refresh-btn"
                    >
                        Refresh
                    </button>
                </div>
            </div>
            
            {filteredAndSortedItems.length === 0 ? (
                <div className="no-items">
                    {filter ? 'No items match your filter.' : 'No items found.'}
                </div>
            ) : (
                <div className="items-grid">
                    {filteredAndSortedItems.map(item => (
                        <ItemCard
                            key={item.id}
                            item={item}
                            onSelect={onItemSelect}
                            onDelete={handleDeleteItem}
                        />
                    ))}
                </div>
            )}
        </div>
    );
};

export default ItemList;
