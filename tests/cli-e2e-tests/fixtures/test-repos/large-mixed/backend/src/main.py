"""Main backend application."""
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Optional
import asyncio
import logging

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="Test API", version="1.0.0")

class Item(BaseModel):
    id: Optional[int] = None
    name: str
    description: Optional[str] = None
    price: float
    category: str

class ItemCreate(BaseModel):
    name: str
    description: Optional[str] = None
    price: float
    category: str

class ItemUpdate(BaseModel):
    name: Optional[str] = None
    description: Optional[str] = None
    price: Optional[float] = None
    category: Optional[str] = None

# In-memory storage (don't do this in production!)
items_db: List[Item] = []
next_id = 1

@app.get("/")
async def root():
    return {"message": "Test API is running"}

@app.get("/items", response_model=List[Item])
async def get_items(category: Optional[str] = None, limit: int = 100):
    """Get all items, optionally filtered by category."""
    filtered_items = items_db
    if category:
        filtered_items = [item for item in items_db if item.category.lower() == category.lower()]
    return filtered_items[:limit]

@app.get("/items/{item_id}", response_model=Item)
async def get_item(item_id: int):
    """Get a specific item by ID."""
    for item in items_db:
        if item.id == item_id:
            return item
    raise HTTPException(status_code=404, detail="Item not found")

@app.post("/items", response_model=Item)
async def create_item(item: ItemCreate):
    """Create a new item."""
    global next_id
    
    # Validate data
    if item.price <= 0:
        raise HTTPException(status_code=400, detail="Price must be positive")
    
    if not item.name.strip():
        raise HTTPException(status_code=400, detail="Name cannot be empty")
    
    # Check for duplicate names
    for existing_item in items_db:
        if existing_item.name.lower() == item.name.lower():
            raise HTTPException(status_code=400, detail="Item with this name already exists")
    
    new_item = Item(
        id=next_id,
        name=item.name.strip(),
        description=item.description.strip() if item.description else None,
        price=item.price,
        category=item.category.strip()
    )
    
    items_db.append(new_item)
    next_id += 1
    
    logger.info(f"Created item: {new_item.name} (ID: {new_item.id})")
    return new_item

@app.put("/items/{item_id}", response_model=Item)
async def update_item(item_id: int, item_update: ItemUpdate):
    """Update an existing item."""
    for i, existing_item in enumerate(items_db):
        if existing_item.id == item_id:
            # Create updated item
            updated_data = existing_item.dict()
            update_data = item_update.dict(exclude_unset=True)
            
            # Validate updates
            if "price" in update_data and update_data["price"] <= 0:
                raise HTTPException(status_code=400, detail="Price must be positive")
            
            if "name" in update_data and not update_data["name"].strip():
                raise HTTPException(status_code=400, detail="Name cannot be empty")
            
            # Check for duplicate names (excluding current item)
            if "name" in update_data:
                for other_item in items_db:
                    if (other_item.id != item_id and 
                        other_item.name.lower() == update_data["name"].lower()):
                        raise HTTPException(status_code=400, detail="Item with this name already exists")
            
            updated_data.update(update_data)
            updated_item = Item(**updated_data)
            items_db[i] = updated_item
            
            logger.info(f"Updated item: {updated_item.name} (ID: {updated_item.id})")
            return updated_item
    
    raise HTTPException(status_code=404, detail="Item not found")

@app.delete("/items/{item_id}")
async def delete_item(item_id: int):
    """Delete an item."""
    for i, item in enumerate(items_db):
        if item.id == item_id:
            deleted_item = items_db.pop(i)
            logger.info(f"Deleted item: {deleted_item.name} (ID: {deleted_item.id})")
            return {"message": f"Item {item_id} deleted successfully"}
    
    raise HTTPException(status_code=404, detail="Item not found")

# Health check endpoint
@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {
        "status": "healthy",
        "items_count": len(items_db),
        "timestamp": asyncio.get_event_loop().time()
    }

if __name__ == "__main__":
    import uvicorn
    
    # Add some sample data
    sample_items = [
        ItemCreate(name="Laptop", description="High-performance laptop", price=999.99, category="electronics"),
        ItemCreate(name="Coffee Mug", description="Ceramic coffee mug", price=12.99, category="kitchen"),
        ItemCreate(name="Book", description="Programming book", price=39.99, category="books"),
    ]
    
    for sample_item in sample_items:
        asyncio.run(create_item(sample_item))
    
    uvicorn.run(app, host="0.0.0.0", port=8000)
